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
    'ui/setup_window.blp',
    'ui/linked_devices_window.blp',
    'ui/message_item.blp',
    'ui/preferences_window.blp',
    'ui/shortcuts.blp',
    'ui/text_entry.blp',
    'ui/window.blp',
    'ui/submit_captcha_dialog.blp',
    'ui/components/context_menu_bin.blp',
    'ui/components/emoji_picker.blp',
    'ui/components/item_row.blp',
    'ui/components/attachment_audio.blp',
    'ui/components/attachment_file.blp',
    'ui/components/attachment_photo.blp',
    'ui/components/attachment_video.blp',
    'ui/components/item_row.blp',
    'ui/components/time_divider.blp',
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
