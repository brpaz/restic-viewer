# 01 — Project scaffold: empty app shell

**What to build:** A buildable Rust project that boots a GTK4/libadwaita window using an `AdwNavigationSplitView` shell — an empty sidebar pane and an empty detail pane, following GNOME HIG. A Flatpak manifest skeleton builds the app (bundled-restic module comes later, in ticket 10).

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [x] `cargo run` launches a window with `AdwNavigationSplitView`: sidebar pane (placeholder/empty state) and detail pane (placeholder/empty state)
- [x] Window collapses to single-pane navigation at narrow widths (adaptive behavior verified by resizing)
- [x] `flatpak-builder` successfully builds the manifest skeleton and installs; launching the installed build hit a pre-existing host issue unrelated to the app — a stale NFS autofs mount (`/mnt/nas/media`) breaks `bwrap` sandbox creation for any Flatpak app on this machine right now. Build/install verified working; sandboxed launch pending that host fix.
- [x] No restic invocation, Repository logic, or persistence exists yet — this ticket is UI shell only
