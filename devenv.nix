{ pkgs, ... }:

{
  # GTK4/libadwaita apps need a C toolchain for sys crates (gettext-sys, cc-linked
  # gtk4-sys etc.) and pkg-config to find the dev headers below.
  languages.rust = {
    enable = true;
    toolchainFile = ./rust-toolchain.toml;
  };

  packages = [
    pkgs.pkg-config

    # libadwaita >= 1.6 is required (Adw.Spinner) — this input's nixpkgs revision
    # is pinned in devenv.lock, so check `libadwaita --version` after a
    # `devenv update` if the app's minimum version ever moves.
    pkgs.gtk4
    pkgs.libadwaita
    pkgs.libsecret

    # Compiles the .blp UI templates at build time (crates/app/build.rs).
    pkgs.blueprint-compiler

    # Local dev target and what crates/restic-client's integration tests spawn.
    pkgs.restic

    # Building/installing the Flatpak locally (task flatpak:install) and
    # regenerating build-aux/cargo-sources.json.
    pkgs.flatpak-builder
    pkgs.uv

    # The project's command surface — see Taskfile.yml.
    pkgs.go-task

    # Git hooks (lefthook.yml, .commitlintrc.yml).
    pkgs.lefthook
    pkgs.commitlint-rs

    # Packages .deb/.rpm via nfpm.yaml (task package:deb / package:rpm).
    pkgs.nfpm

    # Only needed to build/test the draftsman release-notes tool locally.
    pkgs.go
  ];

  enterShell = ''
    lefthook install >/dev/null
    echo "restic-viewer devenv shell — run 'task' to list available commands."
  '';
}
