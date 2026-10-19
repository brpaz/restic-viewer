cargo_profile = get_option('buildtype') == 'debug' ? 'debug' : 'release'
cargo_args = get_option('buildtype') == 'debug' ? [] : ['--release']

# Blueprint files are compiled and embedded into the binary by build.rs.
custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  env: cargo_env,
  command: [
    cargo,
    'build',
    '--manifest-path', meson.project_source_root() / 'Cargo.toml',
    '-p', 'restic-viewer',
    cargo_args,
    '&&',
    'cp',
    meson.project_build_root() / 'cargo-target' / cargo_profile / meson.project_name(),
    '@OUTPUT@',
  ],
)
