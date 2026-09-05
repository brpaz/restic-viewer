# 02 — Restic client seam + fake + real-CLI fixture test

**What to build:** A single abstraction (trait/interface) that runs a restic subcommand and returns a parsed result, covering the three operations this project needs: list Snapshots, list a Snapshot's Entry tree, and restore. Every other module will depend on this abstraction, never on spawning restic directly. Ship a fake implementation for use by downstream tests, plus one integration test that runs the real (bundled-target) restic binary against a real local Repository fixture created via the actual restic CLI in test setup.

**Blocked by:** None — can start immediately, in parallel with ticket 01.

**Status:** ready-for-agent

- [x] Trait/interface defined for: list Snapshots, list Entry tree (given a Snapshot), restore (given Entries or whole Snapshot + Target Directory)
- [x] Real implementation shells out to restic with `--json`, parses output into the trait's result types
- [x] Fake implementation returns canned results, usable by any downstream module's tests without spawning a process
- [x] Integration test creates a real local restic Repository fixture (via real `restic init`/`restic backup` in test setup), then exercises the real implementation's list-Snapshots and list-Entry-tree calls against it, asserting the parsed results match
- [x] No UI code depends on this ticket; it is purely the client module + its tests
