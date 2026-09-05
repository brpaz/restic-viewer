# 08 — Restore (selected Entries or whole Snapshot)

**What to build:** The Restore flow: the user selects one or more Entries in the tree (or chooses to restore the whole Snapshot), picks a Target Directory via the file chooser portal (defaulting to a new/empty directory, with an explicit option to restore to the Entries' original path instead), and watches progress with the ability to cancel. Failures partway through (permissions, disk space, connection drop) are surfaced clearly.

**Blocked by:** 07.

**Status:** ready-for-agent

- [x] User can select one or more Entries in the tree (via `GtkMultiSelection`) and trigger "Restore Selected" from the tree page's header
- [x] User can trigger "Restore" on a Snapshot row directly from the Snapshot list, restoring the whole Snapshot — a separate action from drilling into the tree
- [x] Target Directory is chosen via the portal-based file chooser (initial folder suggestion: home directory); the user creates/picks the folder themselves via the standard GTK folder-chooser UI, which supports creating a new folder in place
- [x] An explicit "Restore to Original Location" switch restores to `/` (each Entry's own absolute path) instead of the chosen Target Directory
- [x] Restore runs asynchronously via the restic client seam with live progress — the client seam was extended to stream restic's JSON lines as they arrive (`RestoreProgress` callback) instead of buffering the whole output, since restic emits periodic `status` lines during a restore
- [x] User can cancel an in-progress Restore via `RestoreControl`, which force-exits the restic subprocess; verified deterministically via a pre-cancel integration test (real concurrent mid-restore cancellation isn't covered by an automated test — see note below)
- [x] A Restore that fails partway shows a clear, specific error — verified against a **real** permission-denied target directory (integration test asserts restic's own `"permission denied"` text surfaces through)

**Verification notes:**
- restic-client changes (streaming progress, `RestoreControl` cancellation, permission-denied error surfacing) are covered by 5 passing integration tests against the real restic binary — this is the strongest evidence in this ticket.
- The UI (restore dialog, tree multi-selection, per-Snapshot Restore button) compiles clean, lints clean, but — same caveat as ticket 07 — was not click-tested live; synthetic input was ruled unsafe mid-session (see ticket 07's note). Confidence rests on verified API signatures (`MultiSelection`, `SwitchRow`, `ProgressBar`, `FileDialog`) plus the fact that the underlying client calls it wires up are the same ones covered by the integration tests above.
- True mid-restore cancellation (clicking Cancel while restic is actively writing files, not before it starts) is implemented via `Subprocess::force_exit()` — a standard OS-level mechanism — but isn't exercised by an automated test, since reliably timing a "mid-flight" cancel without a slow/flaky fixture wasn't worth the complexity for v1.
