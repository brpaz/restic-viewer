# Meson as the top-level build, driving Cargo

The Flatpak, the `.deb`/`.rpm` packages and local development each used to run `cargo build` and then copy files into place with their own `install` commands (the Flatpak manifest, `nfpm.yaml` inputs and the Taskfile all listed the desktop file, metainfo and icon separately). Meson is now the single entry point: `meson.build` builds the binary through a `cargo build` custom target, installs the binary, desktop file, metainfo and icon, and registers `cargo test` plus the desktop-file and AppStream validators as `meson test` tests.

- The Flatpak module and both native-package jobs use `meson setup` → `compile` → `install` (`DESTDIR` staging for nfpm, which copies the installed tree), so every channel ships the same layout.
- Blueprint compilation, the embedded GResource and the About-page build info stay in `crates/app/build.rs`. The UI is embedded in the binary, so Meson has no resource bundle to install, and `cargo build` / `cargo clippy` keep working on their own.
- `CARGO_HOME` is deliberately not set by Meson: the Flatpak module points it at the vendored sources from `build-aux/cargo-sources.json`.
- Meson passes `LOCALEDIR` (derived from the install prefix) to Cargo, replacing the hand-set variable in the old package builds.
- Formatting, clippy, `cargo deny` and gitleaks remain plain Cargo/CLI tasks; they don't produce build artifacts.

The project version in `meson.build` is written by hand and must match `[workspace.package].version` in `Cargo.toml`.
