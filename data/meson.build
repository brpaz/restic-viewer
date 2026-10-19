desktop_file = files(app_id + '.desktop')
install_data(desktop_file, install_dir: datadir / 'applications')

if desktop_file_validate.found()
  test('validate-desktop', desktop_file_validate, args: [desktop_file])
endif

metainfo_file = files(app_id + '.metainfo.xml')
install_data(metainfo_file, install_dir: datadir / 'metainfo')

if appstreamcli.found()
  test(
    'validate-metainfo',
    appstreamcli,
    args: ['validate', '--no-net', '--explain', metainfo_file],
  )
endif

install_data(
  'icons/hicolor/scalable/apps' / app_id + '.svg',
  install_dir: datadir / 'icons/hicolor/scalable/apps',
)
