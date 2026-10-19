project(
  'restic-viewer',
  version: '0.1.0',
  meson_version: '>= 0.59',
  license: 'MIT',
)

app_id = 'dev.brunopaz.ResticViewer'

cargo = find_program('cargo')
desktop_file_validate = find_program('desktop-file-validate', required: false)
appstreamcli = find_program('appstreamcli', required: false)

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')
localedir = prefix / get_option('localedir')

# CARGO_HOME is left alone: the Flatpak module points it at the vendored sources.
cargo_env = environment()
cargo_env.set('CARGO_TARGET_DIR', meson.project_build_root() / 'cargo-target')
cargo_env.set('LOCALEDIR', localedir)

subdir('data')
subdir('crates/app')

test(
  'cargo-test',
  cargo,
  args: ['test', '--workspace'],
  env: cargo_env,
  workdir: meson.project_source_root(),
  timeout: 600,
)
