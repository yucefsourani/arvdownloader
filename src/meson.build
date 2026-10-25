cargo = find_program('cargo', required: true)

exe_ext = host_machine.system() == 'windows' ? '.exe' : ''
bin_name = 'arvdownloader' + exe_ext

cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: bin_name,
  console: true,
  install: true,
  install_dir: get_option('bindir'),
  command: [
    'sh',
    '-c',
    'cargo build --manifest-path "@0@/Cargo.toml" --target-dir "@1@/target" --release && cp "@1@/target/release/@2@" "@3@"'.format(
      meson.project_source_root(),
      meson.project_build_root(),
      bin_name,
      '@OUTPUT@',
    ),
  ],
)
