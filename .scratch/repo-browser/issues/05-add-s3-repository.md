# 05 — Add S3-compatible Repository

**What to build:** Extend the "add Repository" flow with an S3-compatible Backend option: endpoint, bucket, and access key ID fields, plus secret access key storage in the keyring, following the same validate-then-save pattern as the other Backends.

**Blocked by:** 03.

**Status:** ready-for-agent

**Found while implementing:** restic defaults to a `--stuck-request-timeout` of 5 minutes for network backends. An unreachable/misconfigured S3 endpoint doesn't fail fast the way Local (instant) or SFTP (~1s, `ssh` connection refused) do — the validation call can hang for minutes with no feedback. Not fixed here; needs a product decision (pass a shorter `--stuck-request-timeout`? add a cancel button on the validation spinner? both?) that applies to every restic call the app makes, not just this one, so it doesn't belong to this ticket alone. Follow-up ticket recommended before shipping S3 support.

- [x] "Add Repository" offers S3-compatible as a Backend choice with fields for endpoint, bucket, and access key ID
- [x] Secret access key is entered and stored in the keyring, never the config file
- [x] The app validates the S3 Repository (via the restic client seam) before saving, same as the Local flow
- [x] On success, the Repository appears in the sidebar and persists across restart (non-secret S3 connection fields in TOML config, secret key in keyring)
- [x] Unreachable endpoint / wrong credentials / wrong bucket shows a clear error and does not add the Repository (eventually — see timeout note above)
