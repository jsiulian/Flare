subdir('icons')
subdir('resources')

# Desktop file
desktop_conf = configuration_data()
desktop_conf.set('appid', application_id)
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

# DBus services
service_conf = configuration_data()
service_conf.set('bindir', bindir)
service_conf.set('appid', application_id)
configure_file(
  input: 'de.schmidhuberj.Flare.service.in',
  output: '@0@.service'.format(application_id),
  configuration: service_conf,
  install: true,
  install_dir: dbusdir
)


# Metainfo
install_data(
  '@0@.metainfo.xml'.format(application_id),
  install_dir: datadir / 'metainfo'
)

# Metainfo
install_data(
  '@0@.gschema.xml'.format(application_id),
  install_dir: datadir / 'glib-2.0' / 'schemas'
)

# Compile schemas
settings_schemas = [ '@0@.gschema.xml'.format(application_id) ]
gnome.compile_schemas(depend_files: files(settings_schemas))
