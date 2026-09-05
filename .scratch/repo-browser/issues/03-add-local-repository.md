# 03 — Add Local Repository, list in sidebar, persist + keyring

**What to build:** The "add Repository" flow for a Local Backend: the user picks a folder via the GTK file chooser portal, enters the Repository Password, the app validates it (via the restic client seam) and stores the Repository in the sidebar list. Non-secret metadata (display name, Backend type, path) is persisted to the TOML config file; the Repository Password goes to the OS keyring via libsecret. The Repository survives an app restart.

**Blocked by:** 01, 02.

**Status:** ready-for-agent

- [x] "Add Repository" UI lets the user choose Local as the Backend and pick a folder via the portal-based file chooser (no `--filesystem=host`)
- [x] User enters the Repository Password; the app validates it by calling the restic client seam against the chosen path before saving
- [x] On success, the Repository appears in the sidebar list with its display name
- [x] Repository metadata (name, Backend type, path) is written to the TOML config file under `$XDG_CONFIG_HOME/restic-gtk/`; the Repository Password is written to the keyring, never to the config file
- [x] Restarting the app reloads the Repository list from config and the sidebar shows it again (password fetched from keyring on demand, not re-prompted unless keyring lookup fails)
- [x] Invalid path or wrong password shows a clear error and does not add the Repository
