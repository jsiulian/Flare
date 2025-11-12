project(
  'flare',
  'rust',
  version: '0.17.3',
  meson_version: '>= 0.59',
  license: 'AGPL-3.0-only',
)

i18n = import('i18n')
gnome = import('gnome')

base_id = 'de.schmidhuberj.Flare'
application_id = 'de.schmidhuberj.Flare'
resources_path = '/de/schmidhuberj/Flare/'

dependency('glib-2.0', version: '>= 2.66')
dependency('gio-2.0', version: '>= 2.70')
dependency('gtk4', version: '>= 4.12.0')
dependency('libadwaita-1', version: '>= 1.5')


glib_compile_resources = find_program('glib-compile-resources', version: '>= 2.66', required: true)
glib_compile_schemas = find_program('glib-compile-schemas', version: '>= 2.66', required: true)
desktop_file_validate = find_program('desktop-file-validate', required: false)
appstreamcli = find_program('appstreamcli', required: false)
cargo = find_program('cargo', version: '>= 1.75', required: true)
libspelling = dependency('libspelling-1', required: false)
gtksourceview_dep = dependency('gtksourceview-5')

version = meson.project_version()

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
localedir = prefix / get_option('localedir')

datadir = prefix / get_option('datadir')
pkgdatadir = datadir / meson.project_name()
iconsdir = datadir / 'icons'
podir = meson.project_source_root() / 'po'
gettext_package = meson.project_name()
dbusdir = datadir / 'dbus-1' / 'services'

if get_option('profile') == 'development'
  profile = 'Devel'
  vcs_tag = run_command('git', 'rev-parse', '--short', 'HEAD', check: true).stdout().strip()
  if vcs_tag == ''
    version_suffix = '-devel'
  else
    version_suffix = '-@0@'.format(vcs_tag)
  endif
else
  profile = ''
  version_suffix = ''
  application_id = base_id
endif

if profile != ''
  application_id = '@0@.@1@'.format(base_id, profile)
else
  application_id = base_id
endif

meson.add_dist_script(
  'build-aux/dist-vendor.sh',
  meson.project_build_root() / 'meson-dist' / meson.project_name() + '-' + version,
  meson.project_source_root()
)

subdir('data')
subdir('po')
subdir('src')

gnome.post_install(
  gtk_update_icon_cache: true,
  glib_compile_schemas: true,
  update_desktop_database: true,
)
