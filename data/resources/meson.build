# Blueprint
blueprints = custom_target('blueprints',
  input: files(
    'ui/attachment.blp',
    'ui/call_message_item.blp',
    'ui/channel_messages.blp',
    'ui/channel_item.blp',
    'ui/channel_item_compact.blp',
    'ui/channel_list.blp',
    'ui/channel_info_dialog.blp',
    'ui/dialog_clear_messages.blp',
    'ui/dialog_unlink.blp',
    'ui/error_dialog.blp',
    'ui/setup_window.blp',
    'ui/linked_devices_window.blp',
    'ui/device_info_item.blp',
    'ui/message_item.blp',
    'ui/preferences_window.blp',
    'ui/shortcuts.blp',
    'ui/text_entry.blp',
    'ui/window.blp',
    'ui/submit_captcha_dialog.blp',
    'ui/new_channel_dialog.blp',
    'ui/components/context_menu_bin.blp',
    'ui/components/emoji_picker.blp',
    'ui/components/attachment_audio.blp',
    'ui/components/attachment_file.blp',
    'ui/components/attachment_image.blp',
    'ui/components/attachment_video.blp',
    'ui/components/item_row.blp',
    'ui/components/time_divider.blp',
    'ui/components/letter_divider.blp',
    'ui/components/label.blp',
    'ui/components/indicators.blp',
  ),
  output: '.',
  command: [find_program('blueprint-compiler', version: '>= 0.12.0'), 'batch-compile', '@OUTPUT@', '@CURRENT_SOURCE_DIR@', '@INPUT@'],
)

# Resources
resources_conf = configuration_data()
resources_conf.set('app-id', application_id)
resources = gnome.compile_resources(
  'resources',
  configure_file(
    input: 'resources.gresource.xml.in',
    output: '@BASENAME@',
    configuration: resources_conf,
  ),
  gresource_bundle: true,
  install: true,
  install_dir: pkgdatadir,
  dependencies: [ appdata, blueprints ],
)
