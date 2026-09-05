use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

/// The storage location type a Repository lives on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Backend {
    Local {
        path: PathBuf,
    },
    Sftp {
        host: String,
        path: String,
        user: String,
    },
    S3 {
        endpoint: String,
        bucket: String,
        access_key_id: String,
    },
}

/// Everything needed to invoke restic against one Repository for a single call.
///
/// Carries secrets in memory only for the duration of the call — callers fetch the
/// Repository Password and any Backend credential from the keyring immediately before
/// building this, and never persist it themselves.
#[derive(Debug, Clone)]
pub struct RepositoryConnection {
    pub backend: Backend,
    pub password: String,
    /// The Backend-specific secret (SFTP password, S3 secret access key). `None` for
    /// backends that authenticate another way (e.g. SFTP via SSH agent/key file).
    pub backend_secret: Option<String>,
}

impl RepositoryConnection {
    pub(crate) fn repository_argument(&self) -> String {
        match &self.backend {
            Backend::Local { path } => path.display().to_string(),
            Backend::Sftp { host, path, user } => format!("sftp:{user}@{host}:{path}"),
            Backend::S3 {
                endpoint, bucket, ..
            } => format!("s3:{endpoint}/{bucket}"),
        }
    }

    pub(crate) fn environment(&self) -> Vec<(String, String)> {
        let mut env = vec![("RESTIC_PASSWORD".to_string(), self.password.clone())];
        if let Backend::S3 { access_key_id, .. } = &self.backend {
            env.push(("AWS_ACCESS_KEY_ID".to_string(), access_key_id.clone()));
            if let Some(secret) = &self.backend_secret {
                env.push(("AWS_SECRET_ACCESS_KEY".to_string(), secret.clone()));
            }
        }
        env
    }
}

/// A single point-in-time backup recorded inside a Repository.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub time: String,
    pub hostname: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub paths: Vec<String>,
}

/// A file or directory inside a Snapshot's tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    pub size: Option<u64>,
    pub mtime: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
}

/// A Restore of either a specific set of Entries or an entire Snapshot.
#[derive(Debug, Clone)]
pub struct RestoreRequest {
    pub snapshot_id: String,
    /// Empty means restore the whole Snapshot.
    pub include_paths: Vec<String>,
    pub target: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct RestoreOutcome {
    pub files_restored: u64,
    pub bytes_restored: u64,
}

/// A progress update emitted while a Restore is running.
#[derive(Debug, Clone, Default)]
pub struct RestoreProgress {
    pub percent_done: f64,
    pub files_restored: u64,
    pub total_files: u64,
    pub bytes_restored: u64,
    pub total_bytes: u64,
}

/// Lets a caller cancel an in-progress Restore from outside the future driving it.
///
/// Cheap to clone (an `Rc` internally) — hand a clone to whatever UI element triggers
/// cancellation (e.g. a Cancel button) while the original drives the `restore` call.
#[derive(Debug, Clone, Default)]
pub struct RestoreControl {
    cancelled: Rc<Cell<bool>>,
    process: Rc<RefCell<Option<gio::Subprocess>>>,
}

impl RestoreControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. If the restic process has already been spawned, it is
    /// force-exited immediately; otherwise the flag alone stops it before it starts.
    pub fn cancel(&self) {
        self.cancelled.set(true);
        if let Some(process) = self.process.borrow().as_ref() {
            process.force_exit();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.get()
    }

    pub(crate) fn set_process(&self, process: gio::Subprocess) {
        *self.process.borrow_mut() = Some(process);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResticError {
    #[error("failed to spawn restic: {0}")]
    Spawn(#[from] glib::Error),
    #[error("restic exited with {code:?}: {stderr}")]
    NonZeroExit { code: Option<i32>, stderr: String },
    #[error("failed to parse restic output: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("restore was cancelled")]
    Cancelled,
}
