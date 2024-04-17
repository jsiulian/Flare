subdir('icons')
subdir('resources')

# Desktop file
desktop_conf = configuration_data()
desktop_conf.set('app-id', application_id)
desktop_conf.set('bindir', bindir)
desktop_file = i18n.merge_file(
  input: configure_file(
    input: 'de.schmidhuberj.Flare.desktop.in.in',
    output: 'de.schmidhuberj.Flare.desktop.in',
    configuration: desktop_conf
  ),
  output: '@0@.desktop'.format(application_id),
  type: 'desktop',
  po_dir: podir,
  install: true,
  install_dir: join_paths(datadir, 'applications')
)

if desktop_file_validate.found()
  test('Validate desktop file', desktop_file_validate, args: [desktop_file])
endif

# DBus services
service_conf = configuration_data()
service_conf.set('bindir', bindir)
service_conf.set('app-id', application_id)
configure_file(
  input: 'de.schmidhuberj.Flare.service.in',
  output: '@0@.service'.format(application_id),
  configuration: service_conf,
  install: true,
  install_dir: dbusdir
)


# Metainfo
appdata_conf = configuration_data()
appdata_conf.set('base-id', base_id)
appdata_conf.set('app-id', application_id)
appdata_conf.set('gettext-package', gettext_package)
appdata = i18n.merge_file(
  input: configure_file(
    input: '@0@.metainfo.xml.in.in'.format(base_id),
    output: '@0@.metainfo.xml.in'.format(application_id),
    configuration: appdata_conf,
  ),
  output: '@0@.metainfo.xml'.format(application_id),
  po_dir: podir,
  install: true,
  install_dir: datadir / 'metainfo'
)

if appstreamcli.found()
  test('Validate appstream file', appstreamcli,
     args: ['validate', '--no-net', '--explain', appdata])
 endif

# Schemas
install_data(
  '@0@.gschema.xml'.format(base_id),
  install_dir: datadir / 'glib-2.0' / 'schemas'
)

# Compile schemas
settings_schemas = [ '@0@.gschema.xml'.format(base_id) ]
gnome.compile_schemas(depend_files: files(settings_schemas))
