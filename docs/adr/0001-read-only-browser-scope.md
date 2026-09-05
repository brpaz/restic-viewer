# Scope: read-only Repository browsing and Restore only

restic already has mature GUIs for full backup management. The driver for this project is a native GTK4/libadwaita experience, not a missing restic feature. We scoped the app to attaching to existing Repositories, browsing Snapshots and their Entries, and Restore — no `init`, no running backups, no scheduling, no `forget`/prune, no `mount`. Backups keep being run by cron/CLI/another tool; this app is purely a viewer and restore path. Revisit only if the read-only scope proves too limiting in practice.
