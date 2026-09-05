//! The restic client seam: the one boundary through which this project talks to restic.
//!
//! Everything else in the app depends on the [`ResticClient`] trait, never on spawning
//! restic directly. [`RealResticClient`] shells out to the restic binary; [`FakeResticClient`]
//! is the stand-in every other module's tests build against.

mod real;
mod types;

pub use real::RealResticClient;
pub use types::{
    Backend, Entry, EntryKind, RepositoryConnection, ResticError, RestoreControl, RestoreOutcome,
    RestoreProgress, RestoreRequest, Snapshot,
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
        }]));

        let result = glib::MainContext::new().block_on(fake.list_snapshots(&connection()));

        assert_eq!(result.unwrap()[0].id, "abc123");
    }
}
