# restic-gtk

A native GNOME (GTK4/libadwaita) application for browsing and restoring from existing restic repositories. It does not create backups — it is a viewer and restore tool for repositories backed up elsewhere (cron, CLI, another machine).

## Language

**Repository**:
An existing restic repository the app is attached to for browsing. Always created and backed up outside the app (via `restic init`/`restic backup` run elsewhere); the app never initializes one.
_Avoid_: Repo (informal only), Vault, Backup set

**Backend**:
The storage location type a Repository lives on — Local, SFTP, or S3-compatible in v1.
_Avoid_: Provider, Destination

**Snapshot**:
A single point-in-time backup recorded inside a Repository. Snapshots are produced by backups run outside the app; the app only lists and browses them.
_Avoid_: Backup, Version

**Entry**:
A file or directory inside a Snapshot's tree, as shown while browsing.
_Avoid_: Item, Node

**Restore**:
Copying one or more selected Entries, or an entire Snapshot, out of a Repository into a Target Directory.
_Avoid_: Recover, Extract

**Target Directory**:
The destination chosen for a Restore. Defaults to a new, user-chosen directory; the original path may be chosen explicitly instead.
_Avoid_: Destination, Output path

**Repository Password**:
The secret that unlocks a Repository's index and data. Stored in the OS keyring (libsecret), never in the app's config file.
_Avoid_: Passphrase, Key (Key collides with backend access keys, e.g. S3 credentials)
