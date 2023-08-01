subdir('ui')

# Blueprint
blueprints = custom_target('blueprints',
  input: files(
    'ui/about.blp',
    'ui/attachment.blp',
    'ui/call_message_item.blp',
    'ui/channel_messages.blp',
    'ui/channel_item.blp',
    'ui/channel_list.blp',
    'ui/channel_info_dialog.blp',
    'ui/dialog_clear_messages.blp',
    'ui/dialog_unlink.blp',
    'ui/error_dialog.blp',
    'ui/link_window.blp',
    'ui/message_item.blp',
    'ui/preferences_window.blp',
    'ui/shortcuts.blp',
    'ui/text_entry.blp',
    'ui/window.blp',
    'ui/submit_captcha_dialog.blp',
    'ui/components/context_menu_bin.blp',
    'ui/components/emoji_picker.blp',
  ),
  output: '.',
  command: [find_program('blueprint-compiler'), 'batch-compile', '@OUTPUT@', '@CURRENT_SOURCE_DIR@', '@INPUT@'],
)

# Resources
resources = gnome.compile_resources(
  'resources',
  'resources.gresource.xml',
  gresource_bundle: true,
  install: false,
  install_dir: pkgdatadir,
  dependencies: blueprints,
)
