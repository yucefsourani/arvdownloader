fs = import('fs')

install_data(
  'com.github.yucefsourani.arvdownloader.desktop',
  install_dir: get_option('datadir') / 'applications',
)

install_data(
  'com.github.yucefsourani.arvdownloader.metainfo.xml',
  install_dir: get_option('datadir') / 'metainfo',
)

if fs.is_file('icons/com.github.yucefsourani.arvdownloader.svg')
  install_data(
    'icons/com.github.yucefsourani.arvdownloader.svg',
    install_dir: get_option('datadir') / 'icons/hicolor/scalable/apps',
  )
endif

foreach size : ['16', '24', '32', '48', '64', '128', '256', '512']
  png_rel = 'icons/hicolor/@0@x@0@/apps/com.github.yucefsourani.arvdownloader.png'.format(size)
  if fs.is_file(png_rel)
    install_data(
      png_rel,
      install_dir: get_option('datadir') / 'icons/hicolor/@0@x@0@/apps'.format(size),
    )
  endif
endforeach
