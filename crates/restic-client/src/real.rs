use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use gio::prelude::*;

use crate::ResticClient;
use crate::types::{
    Entry, EntryKind, RepositoryConnection, ResticError, RestoreControl, RestoreOutcome,
    RestoreProgress, RestoreRequest, Snapshot,
};

/// Shells out to a restic binary and parses its `--json` output.
pub struct RealResticClient {
    binary: PathBuf,
}

impl RealResticClient {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    /// Uses whatever `restic` is resolved on `$PATH`. Inside the Flatpak sandbox this
    /// naturally resolves to the bundled `/app/bin/restic` — `/app/bin` is on `$PATH`
    /// there — so no separate lookup is needed for the bundled binary.
    pub fn system() -> Self {
        Self::new("restic")
    }

    async fn run(
        &self,
        repo: &RepositoryConnection,
        args: &[String],
    ) -> Result<String, ResticError> {
        let launcher = gio::SubprocessLauncher::new(
            gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE,
        );

        for (key, value) in repo.environment() {
            launcher.setenv(&key, &value, true);
        }
        launcher.setenv("RESTIC_REPOSITORY", repo.repository_argument(), true);

        let mut argv: Vec<std::ffi::OsString> = vec![self.binary.as_os_str().to_owned()];
        argv.extend(args.iter().map(std::ffi::OsString::from));
        let argv_refs: Vec<&std::ffi::OsStr> =
            argv.iter().map(std::ffi::OsString::as_os_str).collect();

        let process = launcher.spawn(&argv_refs)?;
        let (stdout, stderr): (Option<glib::GString>, Option<glib::GString>) =
            process.communicate_utf8_future(None).await?;

        if process.exit_status() != 0 {
            return Err(ResticError::NonZeroExit {
                code: Some(process.exit_status()),
                stderr: stderr.map(|s| s.to_string()).unwrap_or_default(),
            });
        }

        Ok(stdout.map(|s| s.to_string()).unwrap_or_default())
    }
}

#[derive(Deserialize)]
struct NodeLine {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    path: String,
    size: Option<u64>,
    mtime: String,
}

#[derive(Deserialize)]
struct SummaryLine {
    files_restored: u64,
    bytes_restored: u64,
}

#[derive(Deserialize)]
struct StatusLine {
    #[serde(default)]
    percent_done: f64,
    #[serde(default)]
    files_restored: u64,
    #[serde(default)]
    total_files: u64,
    #[serde(default)]
    bytes_restored: u64,
    #[serde(default)]
    total_bytes: u64,
}

impl ResticClient for RealResticClient {
    async fn list_snapshots(
        &self,
        repo: &RepositoryConnection,
    ) -> Result<Vec<Snapshot>, ResticError> {
        let stdout = self
            .run(repo, &["snapshots".to_string(), "--json".to_string()])
            .await?;
        Ok(serde_json::from_str(&stdout)?)
    }

    async fn list_tree(
        &self,
        repo: &RepositoryConnection,
        snapshot_id: &str,
        path: &str,
    ) -> Result<Vec<Entry>, ResticError> {
        let stdout = self
            .run(
                repo,
                &[
                    "ls".to_string(),
                    snapshot_id.to_string(),
                    "--json".to_string(),
                ],
            )
            .await?;

        let wanted_parent = Path::new(path);
        let mut entries = Vec::new();
        for line in stdout.lines() {
            let value: Value = serde_json::from_str(line)?;
            if value.get("message_type").and_then(Value::as_str) != Some("node") {
                continue;
            }
            let node: NodeLine = serde_json::from_value(value)?;
            if Path::new(&node.path).parent() != Some(wanted_parent) {
                continue;
            }
            entries.push(Entry {
                name: node.name,
                path: node.path,
                kind: match node.kind.as_str() {
                    "dir" => EntryKind::Directory,
                    "symlink" => EntryKind::Symlink,
                    _ => EntryKind::File,
                },
                size: node.size,
                mtime: node.mtime,
            });
        }
        Ok(entries)
    }

    async fn restore(
        &self,
        repo: &RepositoryConnection,
        request: &RestoreRequest,
        on_progress: impl Fn(RestoreProgress) + 'static,
        control: RestoreControl,
    ) -> Result<RestoreOutcome, ResticError> {
        let mut args = vec![
            "restore".to_string(),
            request.snapshot_id.clone(),
            "--target".to_string(),
            request.target.display().to_string(),
            "--json".to_string(),
        ];
        for include in &request.include_paths {
            args.push("--include".to_string());
            // restic's --include only matches a directory node exactly (no descendants)
            // when the pattern has no leading '/', even though `restic ls` always reports
            // paths with one — with the leading '/' kept, an exact non-glob match restores
            // nothing.
            args.push(include.strip_prefix('/').unwrap_or(include).to_string());
        }

        if control.is_cancelled() {
            return Err(ResticError::Cancelled);
        }

        let launcher = gio::SubprocessLauncher::new(
            gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_MERGE,
        );
        for (key, value) in repo.environment() {
            launcher.setenv(&key, &value, true);
        }
        launcher.setenv("RESTIC_REPOSITORY", repo.repository_argument(), true);

        let mut argv: Vec<std::ffi::OsString> = vec![self.binary.as_os_str().to_owned()];
        argv.extend(args.iter().map(std::ffi::OsString::from));
        let argv_refs: Vec<&std::ffi::OsStr> =
            argv.iter().map(std::ffi::OsString::as_os_str).collect();

        let process = launcher.spawn(&argv_refs)?;
        control.set_process(process.clone());

        // restic emits one JSON line per event (status updates, then a final summary,
        // or an error) rather than one blob at the end, so this streams stdout line by
        // line as it arrives instead of buffering the whole thing like `run` does —
        // that's what makes live progress and mid-run cancellation possible. stderr is
        // merged into the same stream (STDERR_MERGE) since it isn't JSON and we only
        // need it for the error message on failure.
        let stdout = process
            .stdout_pipe()
            .expect("stdout pipe requested via STDOUT_PIPE");
        let reader = gio::DataInputStream::new(&stdout);

        let mut outcome = RestoreOutcome::default();
        let mut diagnostics = String::new();

        loop {
            let line = match reader.read_line_utf8_future(glib::Priority::DEFAULT).await {
                Ok(Some(line)) => line,
                Ok(None) | Err(_) => break,
            };

            let Ok(value) = serde_json::from_str::<Value>(&line) else {
                diagnostics.push_str(&line);
                diagnostics.push('\n');
                continue;
            };

            match value.get("message_type").and_then(Value::as_str) {
                Some("status") => {
                    if let Ok(status) = serde_json::from_value::<StatusLine>(value) {
                        on_progress(RestoreProgress {
                            percent_done: status.percent_done,
                            files_restored: status.files_restored,
                            total_files: status.total_files,
                            bytes_restored: status.bytes_restored,
                            total_bytes: status.total_bytes,
                        });
                    }
                }
                Some("summary") => {
                    if let Ok(summary) = serde_json::from_value::<SummaryLine>(value) {
                        outcome = RestoreOutcome {
                            files_restored: summary.files_restored,
                            bytes_restored: summary.bytes_restored,
                        };
                    }
                }
                Some("error") => {
                    if let Some(message) = value.pointer("/error/message").and_then(Value::as_str) {
                        diagnostics.push_str(message);
                        diagnostics.push('\n');
                    }
                }
                Some("exit_error") => {
                    if let Some(message) = value.get("message").and_then(Value::as_str) {
                        diagnostics.push_str(message);
                        diagnostics.push('\n');
                    }
                }
                _ => {}
            }
        }

        let _ = process.wait_future().await;

        if control.is_cancelled() {
            return Err(ResticError::Cancelled);
        }

        if process.exit_status() != 0 {
            return Err(ResticError::NonZeroExit {
                code: Some(process.exit_status()),
                stderr: diagnostics,
            });
        }

        Ok(outcome)
    }
}
