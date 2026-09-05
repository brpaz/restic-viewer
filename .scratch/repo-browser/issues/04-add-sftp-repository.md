# 04 — Add SFTP Repository

**What to build:** Extend the "add Repository" flow with an SFTP Backend option: host, remote path, and username fields, following the same validate-then-save pattern as the Local Backend.

**Correction (found while implementing):** restic's SFTP backend has no password mechanism — it shells out to the system `ssh`/`sftp` client, so auth is always via SSH key/agent already configured on the host (restic's own docs: "Passwordless login is important since automatic backups are not possible if the server prompts for credentials"). The original acceptance criteria below assumed a storable SFTP credential; there isn't one. Confirmed with the user: SFTP is passwordless-only for v1 — no Backend secret is stored for it, only the Repository Password (same as every Backend).

**Blocked by:** 03.

**Status:** ready-for-agent

- [x] "Add Repository" offers SFTP as a Backend choice with fields for host, remote path, and username
- [x] No credential field is shown for SFTP — auth relies entirely on the host's existing SSH key/agent setup
- [x] The app validates the SFTP Repository (via the restic client seam, using the Repository Password) before saving, same as the Local flow
- [x] On success, the Repository appears in the sidebar and persists across restart (SFTP connection fields in TOML config; only the Repository Password goes to the keyring)
- [x] Unreachable host / wrong path / SSH auth failure shows a clear error and does not add the Repository (verified restic's real error text against a closed port)
