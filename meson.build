project(
  'arvdownloader',
  'rust',
  version: '0.1.0',
  license: 'GPL-3.0-or-later',
  meson_version: '>= 0.62.0',
  default_options: ['warning_level=2'],
)

app_id = 'com.github.yucefsourani.arvdownloader'

# Verify required native C/GTK libraries via pkg-config (Linux & MSYS2 UCRT64)
dependency('glib-2.0', version: '>= 2.72')
dependency('gio-2.0', version: '>= 2.72')
dependency('gtk4', version: '>= 4.10')
dependency('libadwaita-1', version: '>= 1.4')
dependency('libsoup-3.0', version: '>= 3.0')

subdir('data')
subdir('src')

gnome = import('gnome')
gnome.post_install(
  gtk_update_icon_cache: true,
  update_desktop_database: host_machine.system() != 'windows',
)
