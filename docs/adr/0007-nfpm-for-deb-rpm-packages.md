# nfpm for .deb/.rpm packages, restic as a system dependency

The Flatpak is Restic Viewer's primary distribution channel, but not every user wants Flatpak. We added native `.deb` and `.rpm` packages built with [nfpm](https://nfpm.goreleaser.com/) — a single YAML config (`nfpm.yaml`) produces both formats without needing `dpkg-deb`/`rpmbuild` or per-format packaging scripts.

Unlike the Flatpak (see [ADR-0002](0002-bundle-restic-in-flatpak.md)), these packages don't bundle restic — they declare it as a regular package dependency (`Depends: restic` / `Requires: restic`) and let the distro's own package manager install and update it. Bundling made sense for the Flatpak because the sandbox has no other clean way to reach a restic binary; a native package has no such constraint, and vendoring a Go build step outside the Flatpak's already-established one would just duplicate complexity for no benefit — a native package should behave like every other native package on the system.

Building against Debian stable and Fedora's current release keeps the packages' library requirements aligned with what those distros already ship (see the "test" job's comment in `.github/workflows/ci.yml` for why libadwaita >= 1.6 rules out most stock Ubuntu/older-Debian build environments) — no attempt is made to support distributions whose GTK4/libadwaita stack predates that.
