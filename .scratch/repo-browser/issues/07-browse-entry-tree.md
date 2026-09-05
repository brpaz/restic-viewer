# 07 — Browse a Snapshot's Entry tree

**What to build:** Selecting a Snapshot shows its file tree as expandable/collapsible folders and files (Entries), with size and modification time shown per Entry, so the user can visually navigate to what they're looking for.

**Blocked by:** 06.

**Status:** ready-for-agent

- [x] Selecting a Snapshot loads its root Entry tree via the restic client seam, asynchronously — implemented as an `AdwNavigationView` push from the Snapshot list, using `GtkTreeListModel` + `GtkTreeExpander`
- [x] Folders expand/collapse on interaction; nested folders load their children lazily, only on first expand of that specific row (guarded by a per-node `loaded` flag, since `create_func` runs for every row as soon as it enters the model — eager fetching there would crawl the whole tree)
- [x] Each Entry row shows name, and file size / modification time where applicable
- [~] Large Snapshots remain navigable (no UI freeze) — true by construction (lazy per-row loading means a large tree never triggers more than one `restic ls` call per expanded directory), but not verified against a live fixture — see note below

**Verification gap:** compiles clean, fmt/clippy clean, full test suite green, and the Snapshot-list drill-in point was verified live (correct row rendering). The tree view itself (`entry_tree.rs`) could **not** be click-tested — attempting synthetic input mid-session revealed the active window was the user's own code editor with a real file open, so sending keystrokes/clicks blind was abandoned as too risky. Confidence here rests on line-by-line signature verification against the actual `gtk4`/`libadwaita` crate source (`TreeListModel::new`, `TreeListRow`, `TreeExpander`, `SignalListItemFactory`, `NoSelection`, `ListView`) rather than a live run. Recommend a manual smoke test before relying on this ticket further.
