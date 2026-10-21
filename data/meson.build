data_conf = configuration_data()
data_conf.set('APP_ID', app_id)
data_conf.set('NAME', app_name)

desktop_file = configure_file(
  input: base_app_id + '.desktop.in',
  output: app_id + '.desktop',
  configuration: data_conf,
  install_dir: datadir / 'applications',
)

if desktop_file_validate.found()
  test('validate-desktop', desktop_file_validate, args: [desktop_file])
endif

metainfo_file = configure_file(
  input: base_app_id + '.metainfo.xml.in',
  output: app_id + '.metainfo.xml',
  configuration: data_conf,
  install_dir: datadir / 'metainfo',
)

if appstreamcli.found()
  test(
    'validate-metainfo',
    appstreamcli,
    args: ['validate', '--no-net', '--explain', metainfo_file],
  )
endif

install_data(
  'icons/hicolor/scalable/apps' / base_app_id + '.svg',
  install_dir: datadir / 'icons/hicolor/scalable/apps',
  rename: app_id + '.svg',
)
