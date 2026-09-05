# restic-gtk

A native GNOME (GTK4 + libadwaita) app for browsing and restoring restic backup repositories.

restic-gtk attaches to Repositories you already back up elsewhere (via `restic` CLI, cron, or another tool), lets you browse their Snapshots and file trees, and restores what you need. It doesn't run backups, doesn't schedule anything, and doesn't touch your Repository's data — see [ADR-0001](adr/0001-read-only-browser-scope.md) for why.

## Install

Download the `.flatpak` bundle from the [latest release](https://github.com/brpaz/restic-gtk/releases/latest):

```bash
flatpak install --user restic-gtk.flatpak
flatpak run dev.brunopaz.ResticGtk
```

Building from source, running tests, and the full Taskfile reference live in [CONTRIBUTING](https://github.com/brpaz/restic-gtk/blob/main/CONTRIBUTING.md).

## Architecture

The [Architecture Decisions](adr/index.md) section records the non-obvious, hard-to-reverse choices behind how this app is built — start there before proposing structural changes.
