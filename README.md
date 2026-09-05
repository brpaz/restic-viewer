# Restic Viewer

> A native GNOME (GTK4 + libadwaita) app for browsing and restoring restic backup repositories.

[![CI](https://img.shields.io/github/actions/workflow/status/brpaz/restic-viewer/ci.yml?branch=main&style=for-the-badge)](https://github.com/brpaz/restic-viewer/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge)](LICENSE.md)
[![Flatpak](https://img.shields.io/badge/flatpak-ready-4A90D9?style=for-the-badge&logo=flatpak&logoColor=white)](https://flatpak.org)
[![Vibe coded](https://img.shields.io/badge/vibe-coded-ff69b4?style=for-the-badge)](https://github.com/brpaz/restic-viewer)

Restic Viewer attaches to Repositories you already back up elsewhere (via `restic` CLI, cron, or another tool), lets you browse their Snapshots and file trees, and restores what you need — nothing more. It doesn't run backups, doesn't schedule anything, and doesn't touch your Repository's data.

## Features

- **Browse, don't manage** — attach to an existing Local, SFTP, or S3-compatible Repository; list its Snapshots; walk the file tree inside any of them.
- **Restore what you need** — pick individual files or a whole Snapshot, restore to a new directory or back to the original path.
- **Live progress, real cancel** — restores stream restic's own progress and can be stopped mid-run.
- **Secrets stay in the keyring** — Repository passwords and Backend credentials live in the system keyring (libsecret), never in a config file.
- **Sandboxed by design** — ships as a Flatpak with no `--filesystem=host`; file access goes through the portal file chooser.
- **Bundled restic** — the Flatpak bundles its own restic build, so it works the moment it's installed.

## Install

Download the `.flatpak` bundle from the [latest release](https://github.com/brpaz/restic-viewer/releases/latest):

```bash
flatpak install --user restic-viewer.flatpak
flatpak run dev.brunopaz.ResticViewer
```

## Documentation

Full documentation, including architecture decisions: **[brpaz.github.io/restic-viewer](https://brpaz.github.io/restic-viewer/)**.

## Contributing

Building from source, running tests, and the Taskfile reference live in [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE.md).
