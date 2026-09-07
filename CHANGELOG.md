# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.1.0 - 2026-09-07

### 0.1.0

#### Features

- add devenv shell, recommend it in CONTRIBUTING by [@brpaz](https://github.com/brpaz) ([e464c36](https://github.com/brpaz/restic-viewer/commit/e464c36b2903e4b155d58c370d4edc06f4e00de4))
- add .deb and .rpm packages via nfpm by [@brpaz](https://github.com/brpaz) ([7ed2ee8](https://github.com/brpaz/restic-viewer/commit/7ed2ee842276ec2c8ae5f5af6e1a68e4dc49e80a))
- wire up gettext-based translations by [@brpaz](https://github.com/brpaz) ([444b88d](https://github.com/brpaz/restic-viewer/commit/444b88dd8c55c7323aad37f096320c9167e5048f))
- add Flatpak desktop entry and AppStream metainfo by [@brpaz](https://github.com/brpaz) ([345837a](https://github.com/brpaz/restic-viewer/commit/345837a6c207afbcb4387b502ff779b77762fb2a))
- migrate UI to Blueprint and rename project to Restic Viewer by [@brpaz](https://github.com/brpaz) ([205bf12](https://github.com/brpaz/restic-viewer/commit/205bf1273e8f68d0f499de031656c970e0e78d3f))
- initial restic-gtk implementation by [@brpaz](https://github.com/brpaz) ([30e7940](https://github.com/brpaz/restic-viewer/commit/30e7940b08080fc290802dcc574ccf90dec837d0))

#### Bug Fixes

- auto-regenerate and stage stale flatpak cargo sources on commit by [@brpaz](https://github.com/brpaz) ([5dea3a6](https://github.com/brpaz/restic-viewer/commit/5dea3a65697405cc4ba228aa68ffa77d3ecf4154))
- pin astral-sh/setup-uv to its real latest tag by [@brpaz](https://github.com/brpaz) ([c6a2a36](https://github.com/brpaz/restic-viewer/commit/c6a2a3631560df771aca967139e024cd2def04e1))
- get CI passing again by [@brpaz](https://github.com/brpaz) ([f92ace9](https://github.com/brpaz/restic-viewer/commit/f92ace9ab91bbcc5023bdb4aa5e8e46cd785411c))
- correct selective restore and Snapshot browsing UI bugs by [@brpaz](https://github.com/brpaz) ([f751d17](https://github.com/brpaz/restic-viewer/commit/f751d1767fc3617bf61ab49483770db4abd904f9))
- grant real filesystem access and patch restic for the sandbox by [@brpaz](https://github.com/brpaz) ([6d749aa](https://github.com/brpaz/restic-viewer/commit/6d749aa3990cb8db889576f57e1cd6e13aa353de))
- ship app icon so appstream-compose succeeds by [@brpaz](https://github.com/brpaz) ([0c619fe](https://github.com/brpaz/restic-viewer/commit/0c619fed0c2a2c541f8c0f800ebfbba8fc4fcab3))

#### Documentation

- add Buy Me a Coffee to Support section by [@brpaz](https://github.com/brpaz) ([9f1db41](https://github.com/brpaz/restic-viewer/commit/9f1db41ef7fa32b5fec533dd37abc733da902deb))
- add Support section and FUNDING.yml by [@brpaz](https://github.com/brpaz) ([fd15dce](https://github.com/brpaz/restic-viewer/commit/fd15dcedf4e5715de1f0184554244bc63202281e))
- add Contributors section, reword license line by [@brpaz](https://github.com/brpaz) ([72aa9b9](https://github.com/brpaz/restic-viewer/commit/72aa9b925154205cd58e3fecb89745cc4da51b9d))
- add per-distro install commands, point at flatpak:install by [@brpaz](https://github.com/brpaz) ([888c171](https://github.com/brpaz/restic-viewer/commit/888c171894cf0776f1e4c747e4a1d96e057965fb))
- drop the docs site, note AI authorship by [@brpaz](https://github.com/brpaz) ([e3196a2](https://github.com/brpaz/restic-viewer/commit/e3196a295c0322996491d4659d821d1804910b4b))
- add Build from Source section, drop docs-site link by [@brpaz](https://github.com/brpaz) ([ade503b](https://github.com/brpaz/restic-viewer/commit/ade503bc4797c2ee96522dc13563a9bc407f2e17))

#### Other

- pin the Rust toolchain via rust-toolchain.toml by [@brpaz](https://github.com/brpaz) ([bae5bc8](https://github.com/brpaz/restic-viewer/commit/bae5bc806028abc1a1006779c2ae48730ef336b4))
- drop the pre-push hook by [@brpaz](https://github.com/brpaz) ([34e1dd7](https://github.com/brpaz/restic-viewer/commit/34e1dd75cdea76d0dc0521ba8dd8d5fc4c66ee7f))
- group data/ assets, add pre-push flatpak-sources check by [@brpaz](https://github.com/brpaz) ([181a3ee](https://github.com/brpaz/restic-viewer/commit/181a3ee76929da4927f115cb77b63c37e93d7050))
- pin draftsman action to its latest release, v0.2.3 by [@brpaz](https://github.com/brpaz) ([c1e9a46](https://github.com/brpaz/restic-viewer/commit/c1e9a46aa8a47798abaea3bce760a62e1dd2ed77))
- configure draftsman release notes by [@brpaz](https://github.com/brpaz) ([a32462e](https://github.com/brpaz/restic-viewer/commit/a32462e97809172cbb61a8c1e14f7ba417b38330))
- replace app icon with a distinctive concentric-rings mark by [@brpaz](https://github.com/brpaz) ([47bc79e](https://github.com/brpaz/restic-viewer/commit/47bc79e929e3c48c523c0b6fcd49e7fe88c19cfd))
- upgrade config to best-practices structure (#1) by [@brpaz](https://github.com/brpaz) ([5c4955b](https://github.com/brpaz/restic-viewer/commit/5c4955bb1e23afc2ae74a5161a7d7722352d9645))


---

*Release notes generated by [draftsman](https://brpaz.github.io/draftsman/).*

## [Unreleased]
