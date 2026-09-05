Status: ready-for-agent

# restic-gtk: Repository browser and restore (v1)

## Problem Statement

Existing restic GUIs are functional but don't feel like native GNOME apps — they don't follow GNOME HIG, don't use libadwaita widgets, and look out of place next to the rest of a GNOME desktop. Someone who already runs restic backups (via cron, CLI, or another tool) has no pleasant, native way to look inside their Repositories, see what Snapshots exist, browse what's in them, and pull files back out when needed.

## Solution

A native GTK4 + libadwaita desktop app that attaches to existing restic Repositories (it never creates or backs up into one), lists their Snapshots, lets the user browse the file tree inside any Snapshot, and restores selected Entries or a whole Snapshot to a chosen Target Directory. Nothing else — no backup scheduling, no repository maintenance. See `CONTEXT.md` for the full glossary and `docs/adr/0001-read-only-browser-scope.md` for the scope boundary this spec deliberately holds to.

## User Stories

1. As a restic user, I want to add an existing Repository to the app by pointing at its location, so that I can browse it without touching the CLI.
2. As a restic user, I want to add a Local Repository by picking its folder through a file chooser, so that I don't have to type an absolute path by hand.
3. As a restic user, I want to add an SFTP Repository by entering host, path, and credentials, so that I can browse repositories stored on a remote server.
4. As a restic user, I want to add an S3-compatible Repository by entering endpoint, bucket, and access keys, so that I can browse repositories stored in object storage.
5. As a restic user, I want to enter my Repository Password once when adding a Repository, so that I'm not asked for it on every launch.
6. As a restic user, I want my Repository Password and backend credentials stored in the system keyring, so that they're never sitting in plaintext on disk.
7. As a restic user, I want to be prompted for my Repository Password again if the keyring entry is missing or rejected, so that I can recover from a stale or revoked credential without re-adding the Repository.
8. As a restic user, I want to see a sidebar listing every Repository I've added, so that I can switch between them quickly.
9. As a restic user, I want to rename or remove a Repository from the app's list, so that I can keep the sidebar tidy without affecting the underlying Repository.
10. As a restic user, I want removing a Repository from the app to only forget it locally, so that the actual Repository and its data are never touched.
11. As a restic user, I want to select a Repository and see its list of Snapshots (with date, host, and tags), so that I know what backups exist.
12. As a restic user, I want the Snapshot list to load in the background without freezing the UI, so that a slow remote Backend doesn't make the app feel unresponsive.
13. As a restic user, I want to see a clear error if a Repository can't be reached (wrong credentials, network down, wrong path), so that I understand why nothing loaded.
14. As a restic user, I want to select a Snapshot and browse its file tree as folders and files, so that I can find what I'm looking for visually.
15. As a restic user, I want to expand and collapse folders in the Snapshot tree, so that I can navigate large Snapshots without being overwhelmed.
16. As a restic user, I want to see file size and modification time for Entries in the tree, so that I can identify the right file among similarly named ones.
17. As a restic user, I want to select one or more Entries in the tree and restore just those, so that I don't have to pull back an entire Snapshot for one file.
18. As a restic user, I want to restore an entire Snapshot in one action, so that I can do a full recovery when needed.
19. As a restic user, I want to be asked for a Target Directory before a Restore starts, so that I control where restored files land.
20. As a restic user, I want the Target Directory picker to default to suggesting a new, empty directory, so that I don't accidentally overwrite my current files.
21. As a restic user, I want the option to restore to the Entries' original path instead, so that I can do an in-place recovery when that's actually what I want.
22. As a restic user, I want to see progress while a Restore is running, so that I know it's working and roughly how much is left.
23. As a restic user, I want to be told clearly if a Restore fails partway through (permissions, disk space, connection drop), so that I know what to fix and can retry.
24. As a restic user, I want to cancel a Restore that's in progress, so that I'm not stuck waiting if I picked the wrong thing.
25. As a restic user, I want the app to work fully inside its Flatpak sandbox without me having to grant broad filesystem access, so that installing it doesn't undermine the sandboxing I chose Flatpak for.
26. As a restic user, I want the bundled restic binary to just work without me installing anything separately, so that setup is a single Flatpak install.
27. As a GNOME desktop user, I want the app's window, navigation, and widgets to follow GNOME HIG (adaptive sidebar/detail layout, libadwaita styling), so that it feels consistent with the rest of my desktop.
28. As a restic user, I want the app to remain usable at narrow window widths (sidebar collapses to a single pane), so that I can use it in a tiled or small window.

## Implementation Decisions

- **Restic integration**: all restic access goes through a single client abstraction ("restic client seam") that runs a restic subcommand and returns a parsed result. Every other module (Repository list, Snapshot list, tree browsing, Restore flow) depends only on this abstraction, never on spawning restic directly. This is the one seam in the codebase (per ADR-driven scope, restic is invoked via `restic --json`, never a native binding).
- **Restic binary**: bundled inside the Flatpak build (compiled/vendored as a manifest module), per `docs/adr/0002-bundle-restic-in-flatpak.md`. The app does not check the host `$PATH` or prompt the user to install restic.
- **UI toolkit**: raw `gtk4-rs` + `libadwaita` bindings, manual signal wiring — no relm4 — per `docs/adr/0003-raw-gtk4-rs-over-relm4.md`.
- **Async model**: long-running restic invocations (Snapshot listing, tree listing, Restore) run via `glib::spawn_future_local` over a `gio::Subprocess`, streaming progress/output back into the UI on the GLib main context. No secondary async runtime (e.g. tokio) is introduced.
- **Top-level navigation**: `AdwNavigationSplitView` — sidebar lists Repositories, detail pane shows the selected Repository's Snapshots and, drilling in, the Snapshot's Entry tree. Collapses to single-pane at narrow widths.
- **Repository list persistence**: Repository metadata (display name, Backend type, path/endpoint, non-secret connection fields) is stored in a TOML config file under `$XDG_CONFIG_HOME/restic-gtk/`. No Repository Password or backend secret ever enters this file.
- **Credential storage**: Repository Password and Backend credentials (SFTP password/key, S3 access keys) are stored via libsecret (GNOME Keyring / secret-service), keyed by Repository. Read at the point restic needs to be invoked; never cached in the config file or logged.
- **Backends v1**: Local, SFTP, S3-compatible. Backend-specific connection fields are captured in the "add Repository" flow per Backend type (local: folder path; SFTP: host/path/user; S3-compatible: endpoint/bucket/access key id — secret key goes to the keyring).
- **Repository attach only, no init**: "Add Repository" always points at an existing, already-initialized restic Repository. The app never calls `restic init`.
- **File access inside the sandbox**: any local filesystem path the user picks (Local Repository folder, Restore Target Directory) is chosen via the GTK file chooser portal (`Gio.FileDialog`), never via a blanket `--filesystem=host` Flatpak permission, per `docs/adr/0004-portal-file-access-over-filesystem-host.md`.
- **Restore semantics**: Restore accepts either a specific set of selected Entries or the whole Snapshot, and a Target Directory that defaults to a new/empty directory, with an explicit option to restore to the Entries' original path instead.
- **Out-of-scope operations remain absent from the client abstraction**: the restic client seam only needs to support the subcommands this scope requires (snapshots list, ls/tree listing, restore). It should not grow init/backup/forget/prune/mount support speculatively — see Out of Scope.

## Testing Decisions

- Good tests here exercise observable behavior through the restic client seam's interface (given a canned/fake response for "list snapshots" or "list tree", does the Repository list / Snapshot list / tree view render and behave correctly?) — not internal widget wiring or restic's exact CLI invocation syntax.
- The restic client seam itself gets a small number of integration tests that run the real bundled restic binary against a real local Repository fixture (created and populated using the actual restic CLI in test setup), verifying real JSON output parses into the expected result types. This is the only place real subprocess/real restic behavior is tested.
- All other modules (Repository list management, Snapshot browsing, tree browsing, Restore flow orchestration, credential lookup) are tested against a fake implementation of the restic client seam — no real subprocess, no real filesystem repository, no real keyring in these tests.
- Credential storage/retrieval logic is tested against an in-memory or fake secret-service stub, not the real GNOME Keyring, to keep tests hermetic.
- Config file read/write (Repository list persistence) is tested by writing to a temp directory, not the real `$XDG_CONFIG_HOME`.
- No prior art exists in this repo yet (greenfield project) — these are the first tests written, establishing the seam described above as the project's primary testing boundary going forward.

## Out of Scope

- Running backups (`restic backup`) or any `restic init`.
- Scheduling (no background/recurring runs of any kind).
- `forget`/`prune` or any retention policy configuration.
- `mount` (FUSE-based Repository mounting).
- Cross-snapshot search (`restic find`).
- Snapshot comparison/diffing.
- Backends beyond Local, SFTP, and S3-compatible (Azure, GCS, native REST server, rclone passthrough, etc.).
- Any host-side restic installation, version checking, or PATH detection — restic is bundled, not discovered.

## Further Notes

- This spec covers the entire v1 feature set decided during the `/grill-with-docs` session captured in `CONTEXT.md` and `docs/adr/0001`–`0004`. There is no existing codebase yet — this is the first implementation pass for the project.
- Cross-snapshot search was explicitly deferred (not rejected) during grilling — worth a follow-up spec once basic browsing/restore is solid, per the user stories list not including it.
- If a future spec reintroduces `mount` or scheduled backups, it should reopen `docs/adr/0001-read-only-browser-scope.md` explicitly rather than silently expanding scope.
