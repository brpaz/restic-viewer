# Flatpak keeps broad filesystem access and the SSH agent

[ADR-0004](0004-portal-file-access-over-filesystem-host.md) chose the file chooser portal over `--filesystem=host`. The shipped Flatpak instead grants `--filesystem=home` plus the removable-media locations (`/media`, `/run/media`, `/mnt`) and `--socket=ssh-auth`. Portals alone cannot support what the app does:

- **Restore to Original Location** writes to the absolute paths recorded in a Snapshot, which no portal grants.
- **Local Repositories** often live on external drives or elsewhere in the home directory, and restic needs real paths to them. Document-portal paths break restic's ownership and mode handling on restore (the reason for `build-aux/restic-tolerate-einval-lchown.patch`).
- **SFTP Repositories** need the user's SSH keys and `known_hosts` (inside the home directory) and the SSH agent socket.

The access is deliberately narrower than `--filesystem=host`: system directories stay out of reach, and the app never writes anywhere without a Restore the user started and, for the original location, confirmed. Flathub's linter flags `--filesystem=home`, so the submission carries this justification as a linter exception request.

The `.deb` and `.rpm` packages run unsandboxed and have the same access.
