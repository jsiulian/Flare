use cacao::appkit::menu::{Menu, MenuItem};
use cacao::appkit::{App, AppDelegate};
use cacao::events::EventModifierFlag;
use cacao::notification_center::Dispatcher;
use objc::{class, msg_send, sel, sel_impl};

use super::alert;

use crate::core::channel::{ChannelId, CoreChannel};
use crate::core::message::CoreMessage;

use super::channel_info::ChannelInfoWindow;
use super::linked_devices::{DeviceEntry, LinkedDevicesWindow};
use super::preferences_window::PreferencesWindow;
use super::window::FlareWindow;

/// Messages dispatched from background threads to the main UI thread.
#[derive(Debug)]
pub enum AppMessage {
    SetupPending,
    DisplayQR(Vec<u8>),
    RequestConfirmation,
    SetupFinished,
    SetupError(String),
    ChannelsLoaded(Vec<CoreChannel>),
    MessagesLoaded(ChannelId, Vec<CoreMessage>),
    OlderMessagesLoaded(Vec<CoreMessage>),
    NewMessage(CoreMessage),
    ChannelMessageReceived(ChannelId, Option<String>, u64),
    TypingStarted(ChannelId),
    SearchChanged(String),
    CopySelectedMessage,
    SendButtonPressed,
    AttachButtonPressed,
    LoadMorePressed,
    OpenPreferences,
    OpenLinkedDevices,
    DevicesLoaded(Vec<DeviceEntry>),
    OpenChannelInfo,
    OpenContactPicker,
    SyncContacts,
    UnlinkDevice,
    UnlinkDeviceAndDelete,
    ClearAllMessages,
    ClearChannelMessages,
    BackendActionFailed(String),
    ReactToMessage(String),
    OpenAttachment(libsignal_service::proto::AttachmentPointer),
    ContactPickerSearch(String),
    ReplyToSelectedMessage,
    SetReply(CoreMessage),
    ClearReply,
    RetrySetup,
    AppQuit,
    ReceiptsReceived(Vec<u64>, bool),
    ActivateChannel(usize),
    FocusInput,
    DeleteSelectedMessage,
    MessageDeleted(u64),
    ReloadChannel(ChannelId),
    TypingCleared(ChannelId),
    FocusSearch,
    ShowHelp,
    ShowAbout,
    PasteFile(String),
    PasteImage(Vec<u8>, String),
    CheckPasteClipboard,
    ClearAttachments,
    DownloadAttachment(ChannelId, u64, libsignal_service::proto::AttachmentPointer),
}

pub struct FlareApp {
    window: FlareWindow,
    preferences: PreferencesWindow,
    linked_devices: LinkedDevicesWindow,
    channel_info: ChannelInfoWindow,
}

impl Default for FlareApp {
    fn default() -> Self {
        Self {
            window: FlareWindow::default(),
            preferences: PreferencesWindow::default(),
            linked_devices: LinkedDevicesWindow::default(),
            channel_info: ChannelInfoWindow::default(),
        }
    }
}

impl AppDelegate for FlareApp {
    fn did_finish_launching(&self) {
        log::trace!("Cacao app did finish launching");
        App::set_menu(vec![
            Menu::new(
                "",
                vec![
                    MenuItem::new("About Flare").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::ShowAbout);
                    }),
                    MenuItem::Separator,
                    MenuItem::new("Preferences...").key(",").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::OpenPreferences);
                    }),
                    MenuItem::Separator,
                    MenuItem::Hide,
                    MenuItem::HideOthers,
                    MenuItem::ShowAll,
                    MenuItem::Separator,
                    MenuItem::Quit,
                ],
            ),
            Menu::new(
                "File",
                vec![
                    MenuItem::CloseWindow,
                    MenuItem::Separator,
                    MenuItem::new("Sync Contacts").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::SyncContacts);
                    }),
                    MenuItem::Separator,
                    MenuItem::new("Unlink Device...").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::UnlinkDevice);
                    }),
                    MenuItem::new("Unlink and Delete Data...").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(
                            AppMessage::UnlinkDeviceAndDelete,
                        );
                    }),
                    MenuItem::Separator,
                    MenuItem::new("Clear All Messages...").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::ClearAllMessages);
                    }),
                    MenuItem::new("Clear Conversation Messages...").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(
                            AppMessage::ClearChannelMessages,
                        );
                    }),
                ],
            ),
            Menu::new(
                "Edit",
                vec![
                    MenuItem::Undo,
                    MenuItem::Redo,
                    MenuItem::Separator,
                    MenuItem::Cut,
                    MenuItem::Copy,
                    MenuItem::new("Paste").key("v").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::CheckPasteClipboard);
                    }),
                    MenuItem::SelectAll,
                    MenuItem::Separator,
                    MenuItem::new("Find").key("f").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::FocusSearch);
                    }),
                    MenuItem::new("Copy Message")
                        .key("c")
                        .modifiers(&[EventModifierFlag::Control, EventModifierFlag::Command])
                        .action(|| {
                            App::<FlareApp, AppMessage>::dispatch_main(
                                AppMessage::CopySelectedMessage,
                            );
                        }),
                ],
            ),
            Menu::new(
                "Help",
                vec![
                    MenuItem::new("Keyboard Shortcuts").key("?").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::ShowHelp);
                    }),
                    MenuItem::Separator,
                    MenuItem::new("Report a Problem...").action(|| {
                        super::alert::open_url(
                            "https://gitlab.com/schmiddi-on-mobile/flare/-/issues",
                        );
                    }),
                ],
            ),
            Menu::new(
                "Window",
                vec![
                    MenuItem::Minimize,
                    MenuItem::Zoom,
                    MenuItem::Separator,
                    MenuItem::new("Linked Devices").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::OpenLinkedDevices);
                    }),
                    MenuItem::new("Channel Info").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::OpenChannelInfo);
                    }),
                    MenuItem::new("New Conversation").key("n").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::OpenContactPicker);
                    }),
                    MenuItem::Separator,
                    MenuItem::new("Focus Message Input").key("i").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::FocusInput);
                    }),
                    MenuItem::new("Load More Messages").key("l").action(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::LoadMorePressed);
                    }),
                ],
            ),
            Menu::new("Go", {
                let mut items = vec![MenuItem::Separator];
                for i in 1usize..=9 {
                    let key = i.to_string();
                    items.push(
                        MenuItem::new(&format!("Conversation {}", i))
                            .key(&key)
                            .action(move || {
                                App::<FlareApp, AppMessage>::dispatch_main(
                                    AppMessage::ActivateChannel(i - 1),
                                );
                            }),
                    );
                }
                items
            }),
        ]);

        // Fix "Copy Message" in the Edit menu: cacao may leave it with no direct target,
        // causing macOS auto-validation to grey it out. Replace target/action with our handler.
        unsafe {
            use super::menu_action::create_action_handler;
            use objc::runtime::Object;
            use objc::{class, msg_send, sel, sel_impl};

            let nsapp: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let main_menu: *mut Object = msg_send![nsapp, mainMenu];
            // 0=App, 1=File, 2=Edit
            let edit_item: *mut Object = msg_send![main_menu, itemAtIndex: 2i64];
            let edit_menu: *mut Object = msg_send![edit_item, submenu];
            let _: () = msg_send![edit_menu, setAutoenablesItems: objc::runtime::NO];
            let n: i64 = msg_send![edit_menu, numberOfItems];
            for i in 0..n {
                let item: *mut Object = msg_send![edit_menu, itemAtIndex: i];
                let title: *mut Object = msg_send![item, title];
                let s: *const std::os::raw::c_char = msg_send![title, UTF8String];
                if s.is_null() {
                    continue;
                }
                let rust_str = std::ffi::CStr::from_ptr(s).to_str().unwrap_or("");
                if rust_str == "Copy Message" {
                    let handler = create_action_handler(|| {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::CopySelectedMessage);
                    });
                    let _: () = msg_send![item, setTarget: handler];
                    let _: () = msg_send![item, setAction: sel!(perform:)];
                    let _: () = msg_send![item, setEnabled: objc::runtime::YES];
                    // Do NOT release handler — NSMenuItem target is weak/assign.
                    break;
                }
            }
        }

        super::notifications::request_permission();
        App::activate();
        self.window.show();
        self.window.start_backend();
    }

    fn should_terminate_after_last_window_closed(&self) -> bool {
        !super::preferences_window::run_in_background()
    }
}

impl Dispatcher for FlareApp {
    type Message = AppMessage;

    fn on_ui_message(&self, message: Self::Message) {
        log::trace!("Received UI message: {:?}", message);
        match message {
            AppMessage::SetupPending => {
                self.window.show_setup_pending();
            }
            AppMessage::DisplayQR(png_data) => {
                self.window.show_qr_code(&png_data);
            }
            AppMessage::RequestConfirmation => {
                self.window.show_confirmation_entry();
            }
            AppMessage::SetupFinished => {
                self.window.show_main_view();
            }
            AppMessage::SetupError(err) => {
                self.window.show_error(&err);
            }
            AppMessage::ChannelsLoaded(channels) => {
                self.window.set_channels(channels);
            }
            AppMessage::MessagesLoaded(channel_id, messages) => {
                // Withdraw any pending notifications for this channel now that it's open.
                super::notifications::remove_notifications(&channel_id_to_str(&channel_id));
                self.window.set_messages(channel_id, messages);
            }
            AppMessage::OlderMessagesLoaded(messages) => {
                self.window.prepend_messages(messages);
            }
            AppMessage::NewMessage(message) => {
                if !message.is_outgoing {
                    if let Some(ref channel_id) = message.channel_id {
                        let is_active = self.window.is_active_channel(channel_id);
                        if !is_active && super::preferences_window::notifications_enabled() {
                            // Respect the separate "notify on reactions" toggle.
                            if message.is_reaction
                                && !super::preferences_window::notifications_reactions_enabled()
                            {
                                // Skip notification for reactions when disabled.
                            } else {
                                let title = &message.sender_name;
                                let body = message.body.as_deref().unwrap_or("New message");
                                super::notifications::post_notification(
                                    title,
                                    body,
                                    &channel_id_to_str(channel_id),
                                );
                            }
                        }
                    }
                }
                self.window.add_message(message);
            }
            AppMessage::ChannelMessageReceived(channel_id, body, timestamp) => {
                self.window.update_channel_last_message(channel_id, body, timestamp);
            }
            AppMessage::TypingStarted(channel_id) => {
                self.window.show_typing(channel_id);
            }
            AppMessage::SearchChanged(query) => {
                self.window.filter_channels(&query);
            }
            AppMessage::CopySelectedMessage => {
                if let Some(text) = self.window.selected_message_body() {
                    log::trace!("Copying message: {:?}", text);
                    unsafe {
                        use objc::{class, msg_send, sel, sel_impl};
                        use objc::runtime::Object;
                        use super::menu_action::nsstring;
                        let pb: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
                        let _: () = msg_send![pb, clearContents];
                        let ns_str = nsstring(&text);
                        let _: objc::runtime::BOOL = msg_send![pb, setString: ns_str forType: nsstring("public.utf8-plain-text")];
                    }
                } else {
                    log::trace!("No message selected for copy");
                }
            }
            AppMessage::SendButtonPressed => {
                self.window.handle_send();
            }
            AppMessage::CheckPasteClipboard => {
                unsafe {
                    use objc::runtime::Object;
                    use objc::{class, msg_send, sel};
                    
                    let pasteboard: *mut objc::runtime::Object = msg_send![class!(NSPasteboard), generalPasteboard];
                    
                    // First: get types available on pasteboard
                    let types: *mut objc::runtime::Object = msg_send![pasteboard, types];
                    if types.is_null() {
                        log::trace!("Pasteboard has no types");
                        let app: *mut objc::runtime::Object = msg_send![class!(NSApplication), sharedApplication];
                        let _: () = msg_send![app, sendAction: sel!(paste:) to: std::ptr::null::<()>() from: std::ptr::null::<()>()];
                        return;
                    }
                    
                    // Check if it responds to count
                    let type_count: usize = msg_send![types, count];
                    log::trace!("Pasteboard has {} types", type_count);
                    
                    // Try reading file URLs - this is the proper way to get files from pasteboard
                    let url_class: *mut objc::runtime::Object = msg_send![class!(NSURL), class];
                    let url_arr: *mut objc::runtime::Object = msg_send![class!(NSArray), arrayWithObject: url_class];
                    let url_results: *mut objc::runtime::Object = msg_send![pasteboard, readObjectsForClasses: url_arr options: std::ptr::null::<objc::runtime::Object>()];
                    
                    if !url_results.is_null() {
                        let url_count: usize = msg_send![url_results, count];
                        log::trace!("Got {} URLs from pasteboard", url_count);
                        if url_count > 0 {
                            let mut paths: Vec<String> = Vec::new();
                            for i in 0..url_count {
                                let url: *mut objc::runtime::Object = msg_send![url_results, objectAtIndex: i];
                                if !url.is_null() {
                                    let path: *mut objc::runtime::Object = msg_send![url, path];
                                    if !path.is_null() {
                                        let c_str: *const std::os::raw::c_char = msg_send![path, UTF8String];
                                        let bytes = std::slice::from_raw_parts(c_str as *const u8, 4096);
                                        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                                        let path_string = String::from_utf8_lossy(&bytes[..end]).to_string();
                                        if std::path::Path::new(&path_string).exists() {
                                            paths.push(path_string);
                                        }
                                    }
                                }
                            }
                            if !paths.is_empty() {
                                log::trace!("File URLs from pasteboard: {:?}", paths);
                                self.window.handle_paste_files(paths);
                                return;
                            }
                        }
                    }
                    
                    // Try reading NSString - might contain file path as plain text
                    let string_class: *mut objc::runtime::Object = msg_send![class!(NSString), class];
                    let arr: *mut objc::runtime::Object = msg_send![class!(NSArray), arrayWithObject: string_class];
                    let str_results: *mut objc::runtime::Object = msg_send![pasteboard, readObjectsForClasses: arr options: std::ptr::null::<objc::runtime::Object>()];
                    
                    if !str_results.is_null() {
                        let str_count: usize = msg_send![str_results, count];
                        log::trace!("Got {} strings from pasteboard", str_count);
                            if str_count > 0 {
                                let mut paths: Vec<String> = Vec::new();
                                for i in 0..str_count {
                                    let s: *mut objc::runtime::Object = msg_send![str_results, objectAtIndex: i];
                                    if !s.is_null() {
                                        let c_str: *const std::os::raw::c_char = msg_send![s, UTF8String];
                                        let bytes = std::slice::from_raw_parts(c_str as *const u8, 4096);
                                        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                                        let path_string = String::from_utf8_lossy(&bytes[..end]).to_string();
                                        
                                        if path_string.starts_with('/') && std::path::Path::new(&path_string).exists() {
                                            paths.push(path_string);
                                        }
                                    }
                                }
                                if !paths.is_empty() {
                                    log::trace!("File paths from pasteboard: {:?}", paths);
                                    self.window.handle_paste_files(paths);
                                    return;
                                }
                            }
                    }
                    
                    // Try NSImage
                    let img_class: *mut objc::runtime::Object = msg_send![class!(NSImage), class];
                    let img_arr: *mut objc::runtime::Object = msg_send![class!(NSArray), arrayWithObject: img_class];
                    let img_results: *mut objc::runtime::Object = msg_send![pasteboard, readObjectsForClasses: img_arr options: std::ptr::null::<objc::runtime::Object>()];
                    
                    if !img_results.is_null() {
                        let img_count: usize = msg_send![img_results, count];
                        log::trace!("Got {} images from pasteboard", img_count);
                        if img_count > 0 {
                            let first_img: *mut objc::runtime::Object = msg_send![img_results, firstObject];
                            if !first_img.is_null() {
                                let tiff: *mut objc::runtime::Object = msg_send![first_img, TIFFRepresentation];
                                if !tiff.is_null() {
                                    let bitmap: *mut objc::runtime::Object = msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff];
                                    if !bitmap.is_null() {
                                        let png: *mut objc::runtime::Object = msg_send![bitmap, representationUsingType: 4 properties: std::ptr::null::<objc::runtime::Object>()];
                                        if !png.is_null() {
                                            let len: usize = msg_send![png, length];
                                            if len > 0 {
                                                let bytes: *const std::ffi::c_void = msg_send![png, bytes];
                                                let data = std::slice::from_raw_parts(bytes as *const u8, len).to_vec();
                                                let filename = format!("clipboard_{}.png", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                                                log::trace!("Got image from pasteboard, {} bytes", len);
                                                self.window.handle_paste_image(data, filename);
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // Fallback
                    log::trace!("No supported content found, doing normal paste");
                    let app: *mut objc::runtime::Object = msg_send![class!(NSApplication), sharedApplication];
                    let _: () = msg_send![app, sendAction: sel!(paste:) to: std::ptr::null::<()>() from: std::ptr::null::<()>()];
                }
            }
            AppMessage::AttachButtonPressed => {
                self.window.handle_attach();
            }
            AppMessage::PasteFile(path) => {
                self.window.handle_paste_file(path);
            }
            AppMessage::PasteImage(data, filename) => {
                self.window.handle_paste_image(data, filename);
            }
            AppMessage::ClearAttachments => {
                self.window.handle_clear_attachments();
            }
            AppMessage::DownloadAttachment(channel_id, timestamp, pointer) => {
                self.window.send_command(super::backend::BackendCommand::DownloadAttachment(
                    channel_id,
                    timestamp,
                    pointer,
                ));
            }
            AppMessage::LoadMorePressed => {
                self.window.handle_load_more();
            }
            AppMessage::OpenPreferences => {
                self.preferences.show();
            }
            AppMessage::OpenLinkedDevices => {
                self.linked_devices.show();
                self.window.fetch_devices();
            }
            AppMessage::DevicesLoaded(devices) => {
                self.linked_devices.set_devices(devices);
            }
            AppMessage::OpenChannelInfo => {
                if let Some(channel) = self.window.current_channel_info() {
                    self.channel_info.show_for_channel(&channel);
                }
            }
            AppMessage::OpenContactPicker => {
                self.window.open_contact_picker();
            }
            AppMessage::SyncContacts => {
                self.window.send_command(super::backend::BackendCommand::SyncContacts);
            }
            AppMessage::UnlinkDevice => {
                if alert::confirm(
                    "Unlink Device",
                    "This will unlink this device from your Signal account. Your local messages will be kept.",
                    "Unlink",
                    "Cancel",
                ) {
                    self.window.send_command(super::backend::BackendCommand::UnlinkDevice { keep_data: true });
                }
            }
            AppMessage::UnlinkDeviceAndDelete => {
                if alert::confirm(
                    "Unlink and Delete",
                    "This will unlink this device and delete all local messages. This cannot be undone.",
                    "Unlink and Delete",
                    "Cancel",
                ) {
                    self.window.send_command(super::backend::BackendCommand::UnlinkDevice { keep_data: false });
                }
            }
            AppMessage::ClearAllMessages => {
                if alert::confirm(
                    "Clear All Messages",
                    "This will delete all locally stored messages. This cannot be undone.",
                    "Clear Messages",
                    "Cancel",
                ) {
                    self.window.send_command(super::backend::BackendCommand::ClearAllMessages);
                }
            }
            AppMessage::ClearChannelMessages => {
                self.window.clear_current_channel_messages();
            }
            AppMessage::BackendActionFailed(err) => {
                let result = alert::error_with_report("Action Failed", &err, true);
                if result == Some("report".to_string()) {
                    alert::open_url("https://gitlab.com/schmiddi-on-mobile/flare/-/issues");
                }
            }
            AppMessage::ReactToMessage(emoji) => {
                self.window.handle_react(&emoji);
            }
            AppMessage::OpenAttachment(pointer) => {
                self.window.send_command(super::backend::BackendCommand::OpenAttachment(pointer));
            }
            AppMessage::ContactPickerSearch(query) => {
                self.window.filter_contact_picker(&query);
            }
            AppMessage::ReplyToSelectedMessage => {
                if let Some(msg) = self.window.selected_message() {
                    App::<FlareApp, AppMessage>::dispatch_main(AppMessage::SetReply(msg));
                }
            }
            AppMessage::SetReply(message) => {
                self.window.set_reply(message);
            }
            AppMessage::ClearReply => {
                self.window.clear_reply();
            }
            AppMessage::RetrySetup => {
                self.window.retry_setup();
            }
            AppMessage::AppQuit => {
                std::process::exit(0);
            }
            AppMessage::ReceiptsReceived(timestamps, is_read) => {
                self.window.handle_receipts(&timestamps, is_read);
            }
            AppMessage::ActivateChannel(index) => {
                self.window.activate_channel_at_index(index);
            }
            AppMessage::FocusInput => {
                self.window.focus_input();
            }
            AppMessage::FocusSearch => {
                self.window.focus_search();
            }
            AppMessage::DeleteSelectedMessage => {
                self.window.delete_selected_message();
            }
            AppMessage::MessageDeleted(timestamp) => {
                self.window.remove_message(timestamp);
            }
            AppMessage::ReloadChannel(channel_id) => {
                self.window.reload_channel(channel_id);
            }
            AppMessage::TypingCleared(channel_id) => {
                self.window.clear_typing(channel_id);
            }
            AppMessage::ShowHelp => {
                super::alert::info(
                    "Keyboard shortcuts",
                    "Cmd+F  Find (focus search)\nCmd+N  New conversation\nCmd+1–9  Activate conversation\nCmd+L  Load more messages\nCmd+I  Focus message input\nCtrl+Cmd+C  Copy selected message",
                );
            }
            AppMessage::ShowAbout => {
                let version = env!("CARGO_PKG_VERSION");
                let description = env!("CARGO_PKG_DESCRIPTION");
                super::alert::info(
                    "About Flare",
                    &format!(
                        "Flare – Chat with your friends on Signal\n\n\
Version: {version}\n\n\
{description}\n\n\
Flare is an unofficial app for Signal. It is still in development and doesn't include all the features that the official Signal apps do.\n\n\
Please note that using this application will probably worsen your security compared to using official Signal applications. Use with care when handling sensitive data."
                    ),
                );
            }
        }
    }

    fn on_background_message(&self, _message: Self::Message) {}
}

fn channel_id_to_str(id: &crate::core::channel::ChannelId) -> String {
    match id {
        crate::core::channel::ChannelId::Contact(uuid) => uuid.to_string(),
        crate::core::channel::ChannelId::Group(key) => hex::encode(key),
    }
}

pub fn run() {
    log::trace!("Starting cacao GUI");
    App::new("de.schmidhuberj.Flare", FlareApp::default()).run();
}
