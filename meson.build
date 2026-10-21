project(
  'restic-viewer',
  version: '0.1.0',
  meson_version: '>= 0.59',
  license: 'MIT',
)

base_app_id = 'dev.brunopaz.ResticViewer'
if get_option('profile') == 'development'
  app_id = base_app_id + '.Devel'
  app_name = 'Restic Viewer (Development)'
else
  app_id = base_app_id
  app_name = 'Restic Viewer'
endif

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
cargo_env.set('APP_ID', app_id)

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
