# Contributing

## Types of contributions

- **Bug reports** — [open an issue](https://github.com/brpaz/restic-viewer/issues/new/choose); include your install method (Flatpak or from source), the version/commit shown on the About page, and the Backend involved if relevant.
- **Feature requests** — Restic Viewer is deliberately scoped as a *browser*, not a backup manager (see [ADR-0001](docs/adr/0001-read-only-browser-scope.md)); check that scope before proposing something like scheduling or `forget`/`prune`. Backend support, restore UX, and browsing improvements are all fair game.
- **Code** — see the workflow below. Bug fixes and small, well-scoped features can go straight to a PR; anything that touches architecture (a new Backend transport, a different UI framework, sandbox permissions) should reference or add an ADR first.
- **Documentation** — the docs site (`docs/`) and this file are as much a target for improvement as the code.
- **Testing on real setups** — SFTP/S3 Repositories, different distros, different window managers. The test suite covers Local heavily; real-world coverage of the other Backends is thin.

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- GTK4, libadwaita, and libsecret development headers
  - Fedora: `gtk4-devel libadwaita-devel libsecret-devel`
  - Debian/Ubuntu: `libgtk-4-dev libadwaita-1-dev libsecret-1-dev`
- [`restic`](https://restic.net/) on `$PATH` for local development (the Flatpak bundles its own; native builds resolve whatever `restic` is installed)
- [Task](https://taskfile.dev/) (optional, but every command below is a task)

Building the Flatpak locally additionally requires [`flatpak-builder`](https://docs.flatpak.org/) and [`uv`](https://docs.astral.sh/uv/) (used to regenerate vendored Cargo sources).

Committing additionally requires [Lefthook](https://lefthook.dev/) and [`commitlint-rs`](https://github.com/KeisukeYamashita/commitlint-rs) (`cargo install commitlint-rs`) — see [Git hooks](#git-hooks) below.

## Build from source

```bash
git clone https://github.com/brpaz/restic-viewer.git
cd restic-viewer
task run
```

## Build and run the Flatpak

```bash
task flatpak:install
task flatpak:run
```

## Task reference

| Task | What it does |
| --- | --- |
| `task build` / `task build:release` | Build the app (debug / release) |
| `task run` | Run natively, unsandboxed |
| `task test` | Run the full workspace test suite |
| `task fmt` / `task fmt:check` | Format / check formatting |
| `task lint` | `cargo clippy` with warnings denied |
| `task ci` | Everything CI runs, locally |
| `task flatpak:sources` | Regenerate `build-aux/cargo-sources.json` after touching `Cargo.toml` |
| `task flatpak:sources:check` | Fail if `cargo-sources.json` is stale (what CI runs) |
| `task flatpak:build` / `flatpak:install` / `flatpak:run` | Build / install / run the Flatpak |
| `task hooks:install` | Install the Git hooks (Lefthook) |

Run `task ci` before opening a PR — it's the same fmt/lint/test sequence `.github/workflows/ci.yml` runs.

## Git hooks

```bash
task hooks:install
```

Installs [Lefthook](https://lefthook.dev/)-managed hooks (`lefthook.yml`):

- **pre-commit** — `cargo fmt --check` and `cargo clippy` on staged Rust files.
- **commit-msg** — validates the message against [Conventional Commits](https://www.conventionalcommits.org/) via `commitlint-rs` (`.commitlintrc.yml`).

## Project structure and conventions

- [`CONTEXT.md`](CONTEXT.md) — the domain glossary. Use its terms; if a concept doesn't already have a canonical name there, that's a signal to either use the existing one or add it.
- [`docs/adr/`](docs/adr) — Architecture Decision Records for the hard-to-reverse choices behind how this app is built. Read the relevant ones before proposing structural changes; if your change contradicts one, say so explicitly rather than silently overriding it.
- [`.scratch/`](.scratch) — specs and implementation tickets, one file per ticket (see [`docs/agents/issue-tracker.md`](docs/agents/issue-tracker.md)).
- Dependencies are kept current by [Renovate](renovate.json); `build-aux/cargo-sources.json` must stay in sync with `Cargo.lock` for the Flatpak build to work — CI checks this, `task flatpak:sources` fixes it.

## Commit messages

This project follows [Conventional Commits](https://www.conventionalcommits.org/).
