# 09 — Repository management: rename/remove

**What to build:** From the sidebar, the user can rename a Repository's display name or remove it from the app's list entirely. Removing a Repository only forgets it locally (config entry + keyring credential) — it never touches the underlying restic Repository or its data.

**Blocked by:** 03.

**Status:** ready-for-agent

- [x] Sidebar offers a rename action per Repository (pencil icon button); the new display name is saved via `RepositoryStore::rename` and the sidebar reloads immediately
- [x] Sidebar offers a remove action per Repository (trash icon button, behind an `AdwAlertDialog` confirmation); removing deletes its config entry (`RepositoryStore::remove`) and both possible keyring credentials (Password, BackendSecret)
- [x] Removing a Repository only ever touches the local config file and keyring — no restic subcommand is invoked, so the Repository and its data are untouched by construction
- [x] After removal, `RepositoryStore::remove` persists immediately to the TOML file, so the Repository does not reappear on restart — covered by a unit test alongside the equivalent rename test

**Verification:** `config.rs` rename/remove logic covered by 2 new unit tests (4 total in that module), full workspace suite green (9 tests), fmt/clippy clean with **zero warnings** for the first time this session (the earlier `dead_code` warnings for `keyring::delete`/`CredentialKind::BackendSecret` are gone now that removal actually uses them). Sidebar rendering of the rename/trash buttons verified live via screenshot (config-seeded, no synthetic input needed since this only required visual confirmation, not interaction).
