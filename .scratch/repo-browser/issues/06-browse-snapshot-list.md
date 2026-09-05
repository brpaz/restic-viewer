# 06 — Browse a Repository's Snapshot list

**What to build:** Selecting a Repository in the sidebar loads its Snapshot list (date, host, tags) into the detail pane asynchronously, without freezing the UI. Cover the two failure modes: the Repository is unreachable (network/path/wrong config), and the stored keyring credential is missing or rejected (re-prompt the user for the Repository Password rather than failing silently).

**Blocked by:** 03.

**Status:** ready-for-agent

- [x] Selecting a Repository in the sidebar triggers an async Snapshot list load (via the restic client seam) that does not block the UI thread
- [x] Snapshot list renders date, host, and tags for each Snapshot — verified live: `2026-09-05T11:13:42` / `bruno-laptop · demo`
- [x] If the Repository can't be reached, the detail pane shows a clear error explaining why (not a silent empty list) — uses restic's own error text
- [x] If the keyring credential lookup fails or restic rejects it (exit code 12, "wrong password"), the app re-prompts for the Repository Password inline rather than treating it as a hard failure — verified live
- [x] Switching between Repositories in the sidebar correctly reloads the detail pane for the newly selected one (no stale data from the previous selection) — guarded via a generation counter so a stale async result can't overwrite a newer selection

**Bug found and fixed during live verification:** the sidebar's `row-selected` handler was wired up *after* the initial row population, so GTK's automatic first-row selection fired before any listener existed and the detail pane never loaded on startup. Fixed by connecting the handler before populating the list.
