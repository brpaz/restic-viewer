//! Runs the real, bundled-target restic binary against a real local Repository fixture,
//! built with the actual restic CLI, and checks that RealResticClient parses its output
//! correctly. This is the only place real subprocess/real restic behavior is tested —
//! every other module tests against `FakeResticClient` instead.

use std::path::PathBuf;
use std::process::Command;

use restic_client::{
    Backend, EntryKind, RealResticClient, RepositoryConnection, ResticClient, ResticError,
    RestoreControl,
};

struct Fixture {
    _dir: tempfile::TempDir,
    repo_path: PathBuf,
    source_path: PathBuf,
}

fn build_fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("create tempdir");
    let repo_path = dir.path().join("repo");
    let source_path = dir.path().join("source");
    std::fs::create_dir_all(source_path.join("subdir")).unwrap();
    std::fs::write(source_path.join("file1.txt"), b"hello").unwrap();
    std::fs::write(source_path.join("subdir/file2.txt"), b"world").unwrap();

    let run = |args: &[&str]| {
        let status = Command::new("restic")
            .args(args)
            .env("RESTIC_PASSWORD", "test-password")
            .env("RESTIC_REPOSITORY", &repo_path)
            .status()
            .expect("restic must be installed to run this integration test");
        assert!(status.success(), "restic {args:?} failed");
    };

    run(&["init"]);
    run(&["backup", source_path.to_str().unwrap()]);

    Fixture {
        _dir: dir,
        repo_path,
        source_path,
    }
}

fn connection(fixture: &Fixture) -> RepositoryConnection {
    RepositoryConnection {
        backend: Backend::Local {
            path: fixture.repo_path.clone(),
        },
        password: "test-password".to_string(),
        backend_secret: None,
    }
}

#[test]
fn lists_the_snapshot_just_created() {
    let fixture = build_fixture();
    let repo = connection(&fixture);
    let client = RealResticClient::system();

    let snapshots = glib::MainContext::new()
        .block_on(client.list_snapshots(&repo))
        .expect("list_snapshots");

    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].hostname, whoami_hostname());
    assert_eq!(
        snapshots[0].paths,
        vec![fixture.source_path.to_str().unwrap().to_string()]
    );
}

#[test]
fn lists_direct_children_of_the_source_root() {
    let fixture = build_fixture();
    let repo = connection(&fixture);
    let client = RealResticClient::system();

    let snapshots = glib::MainContext::new()
        .block_on(client.list_snapshots(&repo))
        .expect("list_snapshots");
    let snapshot_id = &snapshots[0].id;

    let entries = glib::MainContext::new()
        .block_on(client.list_tree(&repo, snapshot_id, fixture.source_path.to_str().unwrap()))
        .expect("list_tree");

    let mut names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["file1.txt", "subdir"]);

    let file1 = entries.iter().find(|e| e.name == "file1.txt").unwrap();
    assert_eq!(file1.kind, EntryKind::File);
    assert_eq!(file1.size, Some(5));

    let subdir = entries.iter().find(|e| e.name == "subdir").unwrap();
    assert_eq!(subdir.kind, EntryKind::Directory);
}

#[test]
fn restores_the_whole_snapshot() {
    let fixture = build_fixture();
    let repo = connection(&fixture);
    let client = RealResticClient::system();

    let snapshots = glib::MainContext::new()
        .block_on(client.list_snapshots(&repo))
        .expect("list_snapshots");
    let snapshot_id = snapshots[0].id.clone();

    let target = tempfile::tempdir().unwrap();
    let request = restic_client::RestoreRequest {
        snapshot_id,
        include_paths: vec![],
        target: target.path().to_path_buf(),
    };

    let progress_calls = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let outcome = glib::MainContext::new()
        .block_on(client.restore(
            &repo,
            &request,
            {
                let progress_calls = progress_calls.clone();
                move |progress| progress_calls.borrow_mut().push(progress)
            },
            RestoreControl::new(),
        ))
        .expect("restore");

    // restic counts every restored node (directories included), not just regular files —
    // the two real files plus their ancestor directories down to the filesystem root.
    assert_eq!(outcome.files_restored, 6);
    let restored_file = target
        .path()
        .join(fixture.source_path.strip_prefix("/").unwrap())
        .join("file1.txt");
    assert_eq!(std::fs::read_to_string(restored_file).unwrap(), "hello");
}

#[test]
fn restores_only_the_included_path() {
    let fixture = build_fixture();
    let repo = connection(&fixture);
    let client = RealResticClient::system();

    let snapshots = glib::MainContext::new()
        .block_on(client.list_snapshots(&repo))
        .expect("list_snapshots");
    let snapshot_id = snapshots[0].id.clone();

    let target = tempfile::tempdir().unwrap();
    // Absolute path with a leading '/', matching what `restic ls --json` (and thus
    // entry_tree's selection) reports.
    let included = fixture
        .source_path
        .join("subdir/file2.txt")
        .to_str()
        .unwrap()
        .to_string();
    let request = restic_client::RestoreRequest {
        snapshot_id,
        include_paths: vec![included],
        target: target.path().to_path_buf(),
    };

    let outcome = glib::MainContext::new()
        .block_on(client.restore(&repo, &request, |_| {}, RestoreControl::new()))
        .expect("restore");

    assert!(
        outcome.files_restored > 0,
        "expected the included file to be restored, got {outcome:?}"
    );
    let restored_file = target
        .path()
        .join(fixture.source_path.strip_prefix("/").unwrap())
        .join("subdir/file2.txt");
    assert_eq!(std::fs::read_to_string(restored_file).unwrap(), "world");
    let not_restored = target
        .path()
        .join(fixture.source_path.strip_prefix("/").unwrap())
        .join("file1.txt");
    assert!(!not_restored.exists());
}

#[test]
fn cancelling_before_it_starts_stops_the_restore() {
    let fixture = build_fixture();
    let repo = connection(&fixture);
    let client = RealResticClient::system();

    let snapshots = glib::MainContext::new()
        .block_on(client.list_snapshots(&repo))
        .expect("list_snapshots");
    let snapshot_id = snapshots[0].id.clone();

    let target = tempfile::tempdir().unwrap();
    let request = restic_client::RestoreRequest {
        snapshot_id,
        include_paths: vec![],
        target: target.path().to_path_buf(),
    };

    let control = RestoreControl::new();
    control.cancel();

    let result =
        glib::MainContext::new().block_on(client.restore(&repo, &request, |_| {}, control));

    assert!(
        matches!(result, Err(ResticError::Cancelled)),
        "expected Cancelled, got {result:?}"
    );
    assert!(
        std::fs::read_dir(target.path()).unwrap().next().is_none(),
        "cancelling before restic starts must mean nothing was restored"
    );
}

#[test]
fn a_permission_denied_target_surfaces_restics_own_error_text() {
    use std::os::unix::fs::PermissionsExt;

    if is_root() {
        eprintln!(
            "skipping: running as root, which ignores the permission bits this test relies on"
        );
        return;
    }

    let fixture = build_fixture();
    let repo = connection(&fixture);
    let client = RealResticClient::system();

    let snapshots = glib::MainContext::new()
        .block_on(client.list_snapshots(&repo))
        .expect("list_snapshots");
    let snapshot_id = snapshots[0].id.clone();

    let target = tempfile::tempdir().unwrap();
    std::fs::set_permissions(target.path(), std::fs::Permissions::from_mode(0o000)).unwrap();

    let request = restic_client::RestoreRequest {
        snapshot_id,
        include_paths: vec![],
        target: target.path().to_path_buf(),
    };

    let result = glib::MainContext::new().block_on(client.restore(
        &repo,
        &request,
        |_| {},
        RestoreControl::new(),
    ));

    // Restore the directory's permissions before TempDir's own cleanup runs, regardless
    // of how the assertions below turn out.
    std::fs::set_permissions(target.path(), std::fs::Permissions::from_mode(0o755)).unwrap();

    match result {
        Err(ResticError::NonZeroExit { stderr, .. }) => {
            assert!(
                stderr.contains("permission denied"),
                "expected restic's own permission-denied text, got: {stderr}"
            );
        }
        other => panic!("expected NonZeroExit with restic's permission error, got {other:?}"),
    }
}

fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim() == "0")
        .unwrap_or(false)
}

fn whoami_hostname() -> String {
    Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}
