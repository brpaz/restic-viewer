//! The restic client seam: the one boundary through which this project talks to restic.
//!
//! Everything else in the app depends on the [`ResticClient`] trait, never on spawning
//! restic directly. [`RealResticClient`] shells out to the restic binary; [`FakeResticClient`]
//! is the stand-in every other module's tests build against.

mod real;
mod types;

pub use real::RealResticClient;
pub use types::{
    ArchiveFormat, Backend, CheckReport, DiffChange, DiffEntry, Entry, EntryKind,
    RepositoryConnection, RepositoryStats, ResticError, RestoreControl, RestoreOutcome,
    RestoreProgress, RestoreRequest, SaveRequest, Snapshot, SnapshotSummary,
};

use std::future::Future;

/// Runs restic subcommands against a [`RepositoryConnection`] and returns parsed results.
///
/// Futures returned here are not `Send`: the real implementation is built on GLib/GIO
/// objects bound to the thread-default main context, per
/// `docs/adr/0003-raw-gtk4-rs-over-relm4.md`. The app drives them with
/// `glib::spawn_future_local` on the GLib main thread rather than a multi-threaded executor.
pub trait ResticClient {
    fn list_snapshots(
        &self,
        repo: &RepositoryConnection,
    ) -> impl Future<Output = Result<Vec<Snapshot>, ResticError>>;

    fn list_tree(
        &self,
        repo: &RepositoryConnection,
        snapshot_id: &str,
        path: &str,
    ) -> impl Future<Output = Result<Vec<Entry>, ResticError>>;

    /// Every Entry of a Snapshot. Walks the whole tree, so callers that search repeatedly
    /// should keep the result rather than ask again.
    fn list_all(
        &self,
        repo: &RepositoryConnection,
        snapshot_id: &str,
    ) -> impl Future<Output = Result<Vec<Entry>, ResticError>>;

    /// Lists the paths that differ between two Snapshots, `from` being the older one.
    fn diff(
        &self,
        repo: &RepositoryConnection,
        from_snapshot_id: &str,
        to_snapshot_id: &str,
    ) -> impl Future<Output = Result<Vec<DiffEntry>, ResticError>>;

    /// Reads the contents of one file in a Snapshot into memory. Callers are expected to
    /// bound the file size (see [`Entry::size`]) before asking.
    fn read_file(
        &self,
        repo: &RepositoryConnection,
        snapshot_id: &str,
        path: &str,
    ) -> impl Future<Output = Result<Vec<u8>, ResticError>>;

    /// Size and content totals of the Repository.
    fn stats(
        &self,
        repo: &RepositoryConnection,
    ) -> impl Future<Output = Result<RepositoryStats, ResticError>>;

    /// Runs the read-only `restic check`; with `read_data` it also re-reads every pack,
    /// which can take very long. `control` cancels it like a Restore.
    fn check(
        &self,
        repo: &RepositoryConnection,
        read_data: bool,
        control: RestoreControl,
    ) -> impl Future<Output = Result<CheckReport, ResticError>>;

    /// Streams a file or directory archive straight into `request.destination`. `control`
    /// cancels it like a Restore; a cancelled or failed save leaves no partial file.
    fn save(
        &self,
        repo: &RepositoryConnection,
        request: &SaveRequest,
        control: RestoreControl,
    ) -> impl Future<Output = Result<(), ResticError>>;

    /// Runs a Restore, reporting progress via `on_progress` as restic emits status
    /// updates. `control` can be used by the caller (from outside this future, e.g. a
    /// Cancel button) to stop the Restore early — see [`RestoreControl`].
    fn restore(
        &self,
        repo: &RepositoryConnection,
        request: &RestoreRequest,
        on_progress: impl Fn(RestoreProgress) + 'static,
        control: RestoreControl,
    ) -> impl Future<Output = Result<RestoreOutcome, ResticError>>;
}

/// A canned [`ResticClient`] for tests of every module downstream of this seam.
///
/// Never spawns a process. Returns whatever was configured, in order, one response
/// per call to a given method.
#[derive(Default)]
pub struct FakeResticClient {
    pub snapshots_response: std::sync::Mutex<Option<Result<Vec<Snapshot>, ResticError>>>,
    pub tree_response: std::sync::Mutex<Option<Result<Vec<Entry>, ResticError>>>,
    pub listing_response: std::sync::Mutex<Option<Result<Vec<Entry>, ResticError>>>,
    pub diff_response: std::sync::Mutex<Option<Result<Vec<DiffEntry>, ResticError>>>,
    pub file_response: std::sync::Mutex<Option<Result<Vec<u8>, ResticError>>>,
    pub stats_response: std::sync::Mutex<Option<Result<RepositoryStats, ResticError>>>,
    pub check_response: std::sync::Mutex<Option<Result<CheckReport, ResticError>>>,
    pub save_response: std::sync::Mutex<Option<Result<(), ResticError>>>,
    pub restore_response: std::sync::Mutex<Option<Result<RestoreOutcome, ResticError>>>,
}

impl ResticClient for FakeResticClient {
    async fn list_snapshots(
        &self,
        _repo: &RepositoryConnection,
    ) -> Result<Vec<Snapshot>, ResticError> {
        self.snapshots_response.lock().unwrap().take().expect(
            "FakeResticClient.snapshots_response was not set before list_snapshots was called",
        )
    }

    async fn list_tree(
        &self,
        _repo: &RepositoryConnection,
        _snapshot_id: &str,
        _path: &str,
    ) -> Result<Vec<Entry>, ResticError> {
        self.tree_response
            .lock()
            .unwrap()
            .take()
            .expect("FakeResticClient.tree_response was not set before list_tree was called")
    }

    async fn list_all(
        &self,
        _repo: &RepositoryConnection,
        _snapshot_id: &str,
    ) -> Result<Vec<Entry>, ResticError> {
        self.listing_response
            .lock()
            .unwrap()
            .take()
            .expect("FakeResticClient.listing_response was not set before list_all was called")
    }

    async fn diff(
        &self,
        _repo: &RepositoryConnection,
        _from_snapshot_id: &str,
        _to_snapshot_id: &str,
    ) -> Result<Vec<DiffEntry>, ResticError> {
        self.diff_response
            .lock()
            .unwrap()
            .take()
            .expect("FakeResticClient.diff_response was not set before diff was called")
    }

    async fn read_file(
        &self,
        _repo: &RepositoryConnection,
        _snapshot_id: &str,
        _path: &str,
    ) -> Result<Vec<u8>, ResticError> {
        self.file_response
            .lock()
            .unwrap()
            .take()
            .expect("FakeResticClient.file_response was not set before read_file was called")
    }

    async fn stats(&self, _repo: &RepositoryConnection) -> Result<RepositoryStats, ResticError> {
        self.stats_response
            .lock()
            .unwrap()
            .take()
            .expect("FakeResticClient.stats_response was not set before stats was called")
    }

    async fn check(
        &self,
        _repo: &RepositoryConnection,
        _read_data: bool,
        _control: RestoreControl,
    ) -> Result<CheckReport, ResticError> {
        self.check_response
            .lock()
            .unwrap()
            .take()
            .expect("FakeResticClient.check_response was not set before check was called")
    }

    async fn save(
        &self,
        _repo: &RepositoryConnection,
        _request: &SaveRequest,
        _control: RestoreControl,
    ) -> Result<(), ResticError> {
        self.save_response
            .lock()
            .unwrap()
            .take()
            .expect("FakeResticClient.save_response was not set before save was called")
    }

    async fn restore(
        &self,
        _repo: &RepositoryConnection,
        _request: &RestoreRequest,
        _on_progress: impl Fn(RestoreProgress) + 'static,
        _control: RestoreControl,
    ) -> Result<RestoreOutcome, ResticError> {
        self.restore_response
            .lock()
            .unwrap()
            .take()
            .expect("FakeResticClient.restore_response was not set before restore was called")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection() -> RepositoryConnection {
        RepositoryConnection {
            backend: Backend::Local {
                path: "/tmp/repo".into(),
            },
            password: "pw".to_string(),
            backend_secret: None,
        }
    }

    #[test]
    fn fake_returns_the_configured_snapshots() {
        let fake = FakeResticClient::default();
        *fake.snapshots_response.lock().unwrap() = Some(Ok(vec![Snapshot {
            id: "abc123".to_string(),
            time: "2026-01-01T00:00:00Z".to_string(),
            hostname: "host".to_string(),
            tags: vec![],
            paths: vec!["/home".to_string()],
            summary: None,
        }]));

        let result = glib::MainContext::new().block_on(fake.list_snapshots(&connection()));

        assert_eq!(result.unwrap()[0].id, "abc123");
    }
}
