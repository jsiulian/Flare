use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use cacao::appkit::window::{Window, WindowConfig, WindowDelegate, WindowStyle};
use cacao::appkit::App;
use cacao::button::Button;
use cacao::geometry::Rect;
use cacao::image::{Image, ImageView};
use cacao::input::TextField;
use cacao::layout::{Layout, LayoutConstraint};
use cacao::listview::ListView;
use cacao::text::Label;
use cacao::text::LineBreakMode;
use cacao::view::View;

use cacao::objc_access::ObjcAccess;

use crate::core::channel::{ChannelId, CoreChannel};
use crate::core::message::CoreMessage;
use crate::core::setup::SetupDecision;

use super::app::{AppMessage, FlareApp};
use super::backend::{self, BackendCommand, BackendState};
use super::channel_list::{new_search_field, ChannelListDelegate, SearchField};
use super::contact_picker::ContactPickerWindow;
use super::message_view::MessageListDelegate;
use super::text_field_action::TextFieldActionHandler;
use super::toolbar::{create_toolbar, FlareToolbar};

/// Represents a pending attachment waiting to be sent
struct PendingAttachment {
    path: std::path::PathBuf,
}

impl PendingAttachment {
    fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }
}

fn set_height_constraint(view: &impl ObjcAccess, height: f64) {
    view.with_backing_obj_mut(|obj| unsafe {
        use objc::{msg_send, sel, sel_impl};
        let constraints: *mut objc::runtime::Object = msg_send![obj, constraints];
        let count: usize = msg_send![constraints, count];
        for i in 0..count {
            let c: *mut objc::runtime::Object = msg_send![constraints, objectAtIndex: i];
            let attr: isize = msg_send![c, firstAttribute];
            // NSLayoutAttributeHeight == 8
            if attr == 8 {
                let _: () = msg_send![c, setConstant: height];
                return;
            }
        }
    });
}

pub struct FlareWindowDelegate {
    content: View,

    // Setup views
    setup_view: View,
    setup_label: Label,
    link_button: Button,
    device_name_field: TextField,

    // QR code view
    qr_view: View,
    qr_image_view: ImageView,
    qr_label: Label,

    // Confirmation view
    confirm_view: View,
    confirm_label: Label,
    confirm_field: TextField,
    confirm_button: Button,

    // Main chat view
    main_view: View,
    sidebar: View,
    search_field: SearchField,
    channel_list: ListView<ChannelListDelegate>,
    sidebar_divider: View,
    detail: View,
    conversation_header: Label,
    detail_header_sep: View,
    detail_placeholder: Label,
    load_more_bar: View,
    load_more_button: Button,
    message_list: ListView<MessageListDelegate>,
    typing_label: Label,
    input_separator: View,
    reply_bar: View,
    reply_label: Label,
    reply_close_button: Button,
    input_bar: View,
    attach_button: Button,
    attachment_preview_bar: View,
    attachment_label: Label,
    attachment_clear_button: Button,
    emoji_button: Button,
    input_field: TextField,
    send_button: Button,

    // Error view
    error_view: View,
    error_label: Label,
    error_retry_button: Button,

    // Toolbar (kept alive here)
    toolbar: Option<FlareToolbar>,

    // State
    backend_state: Arc<BackendState>,
    current_channel: RefCell<Option<ChannelId>>,
    current_channel_data: RefCell<Option<CoreChannel>>,
    loaded_message_count: RefCell<usize>,
    input_action_handler: Option<TextFieldActionHandler>,
    contact_picker: ContactPickerWindow,
    drafts: RefCell<HashMap<ChannelId, String>>,
    current_reply: RefCell<Option<CoreMessage>>,
    pending_attachments: RefCell<Vec<PendingAttachment>>,
}

impl FlareWindowDelegate {
    fn new() -> Self {
        let backend_state = Arc::new(BackendState::default());
        Self {
            content: View::new(),
            setup_view: View::new(),
            setup_label: Label::new(),
            link_button: Button::new("Link Device"),
            device_name_field: TextField::new(),
            qr_view: View::new(),
            qr_image_view: ImageView::new(),
            qr_label: Label::new(),
            confirm_view: View::new(),
            confirm_label: Label::new(),
            confirm_field: TextField::new(),
            confirm_button: Button::new("Confirm"),
            main_view: View::new(),
            sidebar: View::new(),
            search_field: new_search_field(),
            channel_list: ListView::with(ChannelListDelegate::new(backend_state.clone())),
            sidebar_divider: View::new(),
            detail: View::new(),
            conversation_header: Label::new(),
            detail_header_sep: View::new(),
            detail_placeholder: Label::new(),
            load_more_bar: View::new(),
            load_more_button: Button::new("Load More"),
            message_list: ListView::with(MessageListDelegate::default()),
            typing_label: Label::new(),
            input_separator: View::new(),
            reply_bar: View::new(),
            reply_label: Label::new(),
            reply_close_button: Button::new("✕"),
            input_bar: View::new(),
            attach_button: Button::new("+"),
            attachment_preview_bar: View::new(),
            attachment_label: Label::new(),
            attachment_clear_button: Button::new("Clear"),
            emoji_button: Button::new(""),
            input_field: TextField::new(),
            send_button: Button::new("Send"),
            error_view: View::new(),
            error_label: Label::new(),
            error_retry_button: Button::new("Try Again"),
            contact_picker: ContactPickerWindow::new(backend_state.clone()),
            toolbar: None,
            backend_state,
            current_channel: RefCell::new(None),
            current_channel_data: RefCell::new(None),
            loaded_message_count: RefCell::new(0),
            input_action_handler: None,
            drafts: RefCell::new(HashMap::new()),
            current_reply: RefCell::new(None),
            pending_attachments: RefCell::new(Vec::new()),
        }
    }

    fn show_only(&self, visible: &View) {
        for view in [
            &self.setup_view,
            &self.qr_view,
            &self.confirm_view,
            &self.main_view,
            &self.error_view,
        ] {
            view.set_hidden(!std::ptr::eq(view, visible));
        }
    }

    fn add_pending_attachment(&self, path: std::path::PathBuf) {
        // Check if this is a file attachment (not an image)
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        let is_file = !matches!(
            extension.as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "heic" | "webp"
        );

        if is_file {
            // "Send alone" rule: file attachments clear other attachments
            // First send any existing pending attachments
            self.send_pending_attachments();
        }

        let attachment = PendingAttachment::new(path);
        self.pending_attachments.borrow_mut().push(attachment);
        self.update_attachment_preview_bar();
    }

    fn send_pending_attachments(&self) {
        let channel_id = self.current_channel.borrow().clone();
        if let Some(channel_id) = channel_id {
            let attachments: Vec<_> = self.pending_attachments.borrow_mut().drain(..).collect();
            for attachment in attachments {
                if let Some(tx) = self.backend_state.command_tx.lock().unwrap().as_ref() {
                    let _ = tx.unbounded_send(BackendCommand::SendAttachment(
                        channel_id.clone(),
                        attachment.path,
                    ));
                }
            }
        }
        self.clear_attachment_preview_bar();
    }

    fn update_attachment_preview_bar(&self) {
        let attachments = self.pending_attachments.borrow();
        if attachments.is_empty() {
            self.attachment_preview_bar.set_hidden(true);
            self.attachment_label.set_hidden(true);
            self.attachment_clear_button.set_hidden(true);
            set_height_constraint(&self.attachment_preview_bar, 0.0);
        } else {
            self.attachment_preview_bar.set_hidden(false);
            self.attachment_label.set_hidden(false);
            self.attachment_clear_button.set_hidden(false);
            set_height_constraint(&self.attachment_preview_bar, 40.0);
            set_view_alpha(&self.attachment_preview_bar, 0.15);
            self.attachment_label
                .set_line_break_mode(LineBreakMode::TruncateTail);

            // Show filenames
            let names: Vec<String> = attachments
                .iter()
                .map(|a| {
                    a.path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("attachment")
                        .to_string()
                })
                .collect();
            self.attachment_label.set_text(&names.join(", "));
        }
    }

    fn clear_attachment_preview_bar(&self) {
        self.pending_attachments.borrow_mut().clear();
        self.attachment_preview_bar.set_hidden(true);
        self.attachment_label.set_hidden(true);
        self.attachment_clear_button.set_hidden(true);
        set_height_constraint(&self.attachment_preview_bar, 0.0);
    }
}

// Appearance helpers

/// Set a view's CALayer background to NSColor.separatorColor (1pt divider).
fn apply_separator_color(view: &View) {
    use cacao::objc_access::ObjcAccess;
    view.with_backing_obj_mut(|obj| unsafe {
        use objc::{class, msg_send, sel, sel_impl};
        let layer: *mut objc::runtime::Object = msg_send![obj, layer];
        let color: *mut objc::runtime::Object = msg_send![class!(NSColor), separatorColor];
        let cg: *mut objc::runtime::Object = msg_send![color, CGColor];
        let _: () = msg_send![layer, setBackgroundColor: cg];
    });
}

/// Set a view's background color with alpha (0.0-1.0).
fn set_view_alpha(view: &View, alpha: f64) {
    use cacao::objc_access::ObjcAccess;
    view.with_backing_obj_mut(|obj| unsafe {
        use objc::{class, msg_send, sel, sel_impl};
        let _: () = msg_send![obj, setWantsLayer: true];
        let layer: *mut objc::runtime::Object = msg_send![obj, layer];
        let ns_color: *mut objc::runtime::Object = msg_send![
            class!(NSColor),
            colorWithWhite: 0.85 alpha: alpha
        ];
        let cg: *mut objc::runtime::Object = msg_send![ns_color, CGColor];
        let _: () = msg_send![layer, setBackgroundColor: cg];
        let _: () = msg_send![layer, setOpaque: false];
    });
}

/// Remove button border so it appears as a plain text link.
fn make_button_borderless(btn: &Button) {
    use cacao::objc_access::ObjcAccess;
    btn.objc.get(|obj| unsafe {
        use objc::{msg_send, sel, sel_impl};
        let obj = obj as *const _ as *mut objc::runtime::Object;
        let _: () = msg_send![obj, setBordered: objc::runtime::NO];
    });
}

/// Replace a button's title with an SF Symbol image (macOS 11+).
fn set_button_sf_symbol(btn: &Button, name: &str) {
    use std::ffi::CString;
    btn.objc.get(|obj| unsafe {
        use objc::{class, msg_send, sel, sel_impl};
        let obj = obj as *const _ as *mut objc::runtime::Object;
        let cs = CString::new(name).unwrap_or_default();
        let ns_name: *mut objc::runtime::Object = {
            let s: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
            msg_send![s, initWithUTF8String: cs.as_ptr()]
        };
        let nil: *mut objc::runtime::Object = std::ptr::null_mut();
        let image: *mut objc::runtime::Object = msg_send![
            class!(NSImage),
            imageWithSystemSymbolName: ns_name
            accessibilityDescription: nil
        ];
        if !image.is_null() {
            let _: () = msg_send![obj, setImage: image];
            // Clear title, remove border, scale image to fit
            let empty: *mut objc::runtime::Object = {
                let s: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
                let cs2 = CString::new("").unwrap_or_default();
                msg_send![s, initWithUTF8String: cs2.as_ptr()]
            };
            let _: () = msg_send![obj, setTitle: empty];
            let _: () = msg_send![obj, setBordered: objc::runtime::NO];
            // NSImageScaleProportionallyDown = 3
            let _: () = msg_send![obj, setImageScaling: 3i32];
        }
    });
}

use super::appearance::{apply_visual_effect, MATERIAL_SIDEBAR};

fn apply_visual_effect_sidebar(sidebar: &View) {
    apply_visual_effect(sidebar, MATERIAL_SIDEBAR);
}

impl WindowDelegate for FlareWindowDelegate {
    const NAME: &'static str = "FlareWindow";

    fn did_load(&mut self, window: Window) {
        window.set_title("Flare");
        window.set_minimum_content_size(600.0, 400.0);
        window.set_content_size(900.0, 600.0);

        // Transparent titlebar for Finder-like appearance
        window.set_titlebar_appears_transparent(true);

        let tb = create_toolbar();
        window.set_toolbar(&tb);
        self.toolbar = Some(tb);

        // --- Setup view ---
        self.setup_label.set_text("Welcome to Flare");
        self.device_name_field.set_placeholder_text("Device name");
        self.device_name_field.set_text("Flare (macOS)");

        let state = self.backend_state.clone();
        self.link_button.set_action(move |_| {
            let device_name = "Flare (macOS)".to_string();
            if let Some(tx) = state.setup_decision_tx.lock().unwrap().take() {
                let _ = tx.send(SetupDecision::Link(
                    libsignal_service::configuration::SignalServers::Production,
                    device_name,
                ));
            }
        });

        self.setup_view.add_subview(&self.setup_label);
        self.setup_view.add_subview(&self.device_name_field);
        self.setup_view.add_subview(&self.link_button);

        LayoutConstraint::activate(&[
            self.setup_label
                .center_x
                .constraint_equal_to(&self.setup_view.center_x),
            self.setup_label
                .top
                .constraint_equal_to(&self.setup_view.top)
                .offset(80.0),
            self.device_name_field
                .center_x
                .constraint_equal_to(&self.setup_view.center_x),
            self.device_name_field
                .top
                .constraint_equal_to(&self.setup_label.bottom)
                .offset(20.0),
            self.device_name_field
                .width
                .constraint_equal_to_constant(250.0),
            self.link_button
                .center_x
                .constraint_equal_to(&self.setup_view.center_x),
            self.link_button
                .top
                .constraint_equal_to(&self.device_name_field.bottom)
                .offset(20.0),
        ]);

        // --- QR code view ---
        self.qr_label.set_text("Scan QR code with your Signal app");
        self.qr_view.add_subview(&self.qr_label);
        self.qr_view.add_subview(&self.qr_image_view);

        LayoutConstraint::activate(&[
            self.qr_label
                .center_x
                .constraint_equal_to(&self.qr_view.center_x),
            self.qr_label
                .top
                .constraint_equal_to(&self.qr_view.top)
                .offset(40.0),
            self.qr_image_view
                .center_x
                .constraint_equal_to(&self.qr_view.center_x),
            self.qr_image_view
                .top
                .constraint_equal_to(&self.qr_label.bottom)
                .offset(20.0),
            self.qr_image_view.width.constraint_equal_to_constant(200.0),
            self.qr_image_view
                .height
                .constraint_equal_to_constant(200.0),
        ]);

        // --- Confirmation view ---
        self.confirm_label
            .set_text("Enter the confirmation code from SMS");
        self.confirm_field.set_placeholder_text("Confirmation code");

        let state = self.backend_state.clone();
        self.confirm_button.set_action(move |_| {
            let code = String::new();
            if let Some(tx) = state.confirm_tx.lock().unwrap().take() {
                let _ = tx.send(code);
            }
        });

        self.confirm_view.add_subview(&self.confirm_label);
        self.confirm_view.add_subview(&self.confirm_field);
        self.confirm_view.add_subview(&self.confirm_button);

        LayoutConstraint::activate(&[
            self.confirm_label
                .center_x
                .constraint_equal_to(&self.confirm_view.center_x),
            self.confirm_label
                .top
                .constraint_equal_to(&self.confirm_view.top)
                .offset(80.0),
            self.confirm_field
                .center_x
                .constraint_equal_to(&self.confirm_view.center_x),
            self.confirm_field
                .top
                .constraint_equal_to(&self.confirm_label.bottom)
                .offset(20.0),
            self.confirm_field.width.constraint_equal_to_constant(250.0),
            self.confirm_button
                .center_x
                .constraint_equal_to(&self.confirm_view.center_x),
            self.confirm_button
                .top
                .constraint_equal_to(&self.confirm_field.bottom)
                .offset(20.0),
        ]);

        // --- Main chat view ---
        use cacao::color::Color;
        use cacao::text::Font;

        // Conversation header (shows channel name when a conversation is open)
        self.conversation_header.set_font(&Font::bold_system(14.));
        self.conversation_header.set_hidden(true);

        // Thin separator below the conversation header
        apply_separator_color(&self.detail_header_sep);

        // Detail placeholder
        self.detail_placeholder.set_text("Select a conversation");
        self.detail_placeholder.set_text_color(Color::SystemGray);
        self.detail_placeholder.set_font(&Font::system(16.));

        self.load_more_button.set_action(move |_| {
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::LoadMorePressed);
        });
        make_button_borderless(&self.load_more_button);
        // Clip subviews so button doesn't bleed outside bar when height=0
        self.load_more_bar.with_backing_obj_mut(|obj| unsafe {
            use objc::{msg_send, sel, sel_impl};
            let _: () = msg_send![obj, setWantsLayer: objc::runtime::YES];
            let layer: *mut objc::runtime::Object = msg_send![obj, layer];
            let _: () = msg_send![layer, setMasksToBounds: objc::runtime::YES];
        });

        self.typing_label.set_font(&Font::system(12.));
        self.typing_label.set_text_color(Color::SystemGray);

        self.input_field.set_placeholder_text("Message");

        self.attach_button.set_action(move |_| {
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::AttachButtonPressed);
        });
        set_button_sf_symbol(&self.attach_button, "paperclip");

        // Clear attachments button
        self.attachment_clear_button.set_action(move |_| {
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::ClearAttachments);
        });
        make_button_borderless(&self.attachment_clear_button);

        // Emoji button — opens native macOS emoji picker
        // Note: The IMK error "messaging the mach port for IMKCFRunLoopWakeUpReliable" may appear
        // if the input field isn't focused - this is a benign macOS-level warning.
        set_button_sf_symbol(&self.emoji_button, "face.smiling");
        self.emoji_button.set_action(move |_| {
            unsafe {
                use objc::{class, msg_send, sel, sel_impl};
                let app: *mut objc::runtime::Object = msg_send![class!(NSApplication), sharedApplication];
                let _: () = msg_send![app, orderFrontCharacterPalette: std::ptr::null::<objc::runtime::Object>()];
            }
        });

        self.send_button.set_action(move |_| {
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::SendButtonPressed);
        });
        // SF Symbol paperplane icon for send button
        set_button_sf_symbol(&self.send_button, "paperplane.fill");

        let action_handler_cell = std::cell::RefCell::new(None);
        self.input_field.objc.with_mut(|obj| {
            *action_handler_cell.borrow_mut() = Some(TextFieldActionHandler::new(obj, || {
                App::<FlareApp, AppMessage>::dispatch_main(AppMessage::SendButtonPressed);
            }));
        });
        self.input_action_handler = action_handler_cell.into_inner();

        // Thin separator above input bar
        apply_separator_color(&self.input_separator);

        // Reply bar (shown when replying to a message)
        self.reply_label.set_font(&Font::system(12.));
        self.reply_label.set_text_color(Color::SystemGray);
        make_button_borderless(&self.reply_close_button);
        self.reply_close_button.set_action(move |_| {
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::ClearReply);
        });
        self.reply_bar.add_subview(&self.reply_label);
        self.reply_bar.add_subview(&self.reply_close_button);
        LayoutConstraint::activate(&[
            self.reply_label
                .leading
                .constraint_equal_to(&self.reply_bar.leading)
                .offset(12.0),
            self.reply_label
                .center_y
                .constraint_equal_to(&self.reply_bar.center_y),
            self.reply_label
                .trailing
                .constraint_equal_to(&self.reply_close_button.leading)
                .offset(-4.0),
            self.reply_close_button
                .trailing
                .constraint_equal_to(&self.reply_bar.trailing)
                .offset(-8.0),
            self.reply_close_button
                .center_y
                .constraint_equal_to(&self.reply_bar.center_y),
            self.reply_close_button
                .width
                .constraint_equal_to_constant(24.0),
            self.reply_close_button
                .height
                .constraint_equal_to_constant(24.0),
        ]);

        self.input_bar.add_subview(&self.attach_button);
        self.input_bar.add_subview(&self.input_field);
        self.input_bar.add_subview(&self.emoji_button);
        self.input_bar.add_subview(&self.send_button);

        LayoutConstraint::activate(&[
            // attach button (paperclip) at leading edge
            self.attach_button
                .leading
                .constraint_equal_to(&self.input_bar.leading)
                .offset(8.0),
            self.attach_button
                .center_y
                .constraint_equal_to(&self.input_bar.center_y),
            self.attach_button.width.constraint_equal_to_constant(32.0),
            self.attach_button.height.constraint_equal_to_constant(32.0),
            // text field fills the middle
            self.input_field
                .leading
                .constraint_equal_to(&self.attach_button.trailing)
                .offset(4.0),
            self.input_field
                .top
                .constraint_equal_to(&self.input_bar.top)
                .offset(6.0),
            self.input_field
                .bottom
                .constraint_equal_to(&self.input_bar.bottom)
                .offset(-6.0),
            // emoji button between input and send
            self.emoji_button
                .leading
                .constraint_equal_to(&self.input_field.trailing)
                .offset(4.0),
            self.emoji_button
                .center_y
                .constraint_equal_to(&self.input_bar.center_y),
            self.emoji_button.width.constraint_equal_to_constant(32.0),
            self.emoji_button.height.constraint_equal_to_constant(32.0),
            // send button at trailing edge
            self.send_button
                .leading
                .constraint_equal_to(&self.emoji_button.trailing)
                .offset(4.0),
            self.send_button
                .trailing
                .constraint_equal_to(&self.input_bar.trailing)
                .offset(-8.0),
            self.send_button
                .center_y
                .constraint_equal_to(&self.input_bar.center_y),
            self.send_button.width.constraint_equal_to_constant(36.0),
            self.send_button.height.constraint_equal_to_constant(36.0),
        ]);

        self.load_more_bar.add_subview(&self.load_more_button);

        self.detail.add_subview(&self.conversation_header);
        self.detail.add_subview(&self.detail_header_sep);
        self.detail.add_subview(&self.detail_placeholder);
        self.detail.add_subview(&self.message_list);
        self.detail.add_subview(&self.load_more_bar);
        self.detail.add_subview(&self.typing_label);
        self.detail.add_subview(&self.input_separator);
        self.detail.add_subview(&self.reply_bar);
        self.detail.add_subview(&self.attachment_preview_bar);
        self.detail.add_subview(&self.attachment_label);
        self.detail.add_subview(&self.attachment_clear_button);
        self.detail.add_subview(&self.input_bar);

        LayoutConstraint::activate(&[
            // Conversation header
            self.conversation_header
                .top
                .constraint_equal_to(&self.detail.top),
            self.conversation_header
                .leading
                .constraint_equal_to(&self.detail.leading)
                .offset(12.0),
            self.conversation_header
                .trailing
                .constraint_equal_to(&self.detail.trailing)
                .offset(-12.0),
            self.conversation_header
                .height
                .constraint_equal_to_constant(38.0),
            // Header separator
            self.detail_header_sep
                .top
                .constraint_equal_to(&self.conversation_header.bottom),
            self.detail_header_sep
                .leading
                .constraint_equal_to(&self.detail.leading),
            self.detail_header_sep
                .trailing
                .constraint_equal_to(&self.detail.trailing),
            self.detail_header_sep
                .height
                .constraint_equal_to_constant(1.0),
            // "Select a conversation" placeholder
            self.detail_placeholder
                .center_x
                .constraint_equal_to(&self.detail.center_x),
            self.detail_placeholder
                .center_y
                .constraint_equal_to(&self.detail.center_y),
            // Load more bar — transparent overlay at top of message list (like attachment_preview_bar)
            self.load_more_bar
                .top
                .constraint_equal_to(&self.detail_header_sep.bottom),
            self.load_more_bar
                .leading
                .constraint_equal_to(&self.detail.leading),
            self.load_more_bar
                .trailing
                .constraint_equal_to(&self.detail.trailing),
            self.load_more_bar.height.constraint_equal_to_constant(0.0),
            self.load_more_button
                .center_x
                .constraint_equal_to(&self.load_more_bar.center_x),
            self.load_more_button
                .center_y
                .constraint_equal_to(&self.load_more_bar.center_y),
            // Message list — starts just below header sep, load_more_bar overlays on top
            self.message_list
                .leading
                .constraint_equal_to(&self.detail.leading),
            self.message_list
                .trailing
                .constraint_equal_to(&self.detail.trailing),
            self.message_list
                .top
                .constraint_equal_to(&self.detail_header_sep.bottom),
            self.message_list
                .bottom
                .constraint_equal_to(&self.typing_label.top),
            // Typing indicator
            self.typing_label
                .leading
                .constraint_equal_to(&self.detail.leading)
                .offset(12.0),
            self.typing_label
                .trailing
                .constraint_equal_to(&self.detail.trailing)
                .offset(-12.0),
            self.typing_label
                .bottom
                .constraint_equal_to(&self.input_separator.top),
            self.typing_label.height.constraint_equal_to_constant(0.0),
            // Input separator
            self.input_separator
                .leading
                .constraint_equal_to(&self.detail.leading),
            self.input_separator
                .trailing
                .constraint_equal_to(&self.detail.trailing),
            self.input_separator
                .bottom
                .constraint_equal_to(&self.reply_bar.top),
            self.input_separator
                .height
                .constraint_equal_to_constant(1.0),
            // Reply bar (hidden by default)
            self.reply_bar
                .leading
                .constraint_equal_to(&self.detail.leading),
            self.reply_bar
                .trailing
                .constraint_equal_to(&self.detail.trailing),
            self.reply_bar
                .bottom
                .constraint_equal_to(&self.input_bar.top),
            self.reply_bar.height.constraint_equal_to_constant(0.0),
            // Attachment preview bar (hidden by default)
            self.attachment_preview_bar
                .leading
                .constraint_equal_to(&self.detail.leading),
            self.attachment_preview_bar
                .trailing
                .constraint_equal_to(&self.detail.trailing),
            self.attachment_preview_bar
                .bottom
                .constraint_equal_to(&self.input_bar.top),
            self.attachment_preview_bar
                .height
                .constraint_equal_to_constant(0.0),
            // Attachment label
            self.attachment_label
                .leading
                .constraint_equal_to(&self.detail.leading)
                .offset(12.0),
            self.attachment_label
                .trailing
                .constraint_equal_to(&self.attachment_clear_button.leading)
                .offset(-8.0),
            self.attachment_label
                .center_y
                .constraint_equal_to(&self.attachment_preview_bar.center_y),
            // Attachment clear button (hidden by default)
            self.attachment_clear_button
                .trailing
                .constraint_equal_to(&self.detail.trailing)
                .offset(-8.0),
            self.attachment_clear_button
                .center_y
                .constraint_equal_to(&self.attachment_preview_bar.center_y),
            self.attachment_clear_button
                .height
                .constraint_equal_to_constant(24.0),
            // Input bar
            self.input_bar
                .leading
                .constraint_equal_to(&self.detail.leading),
            self.input_bar
                .trailing
                .constraint_equal_to(&self.detail.trailing),
            self.input_bar
                .bottom
                .constraint_equal_to(&self.detail.bottom),
            self.input_bar.height.constraint_equal_to_constant(48.0),
        ]);

        // Initially hide chat-specific elements
        self.conversation_header.set_hidden(true);
        self.detail_header_sep.set_hidden(true);
        set_height_constraint(&self.load_more_bar, 0.0);
        self.message_list.set_hidden(true);
        self.typing_label.set_hidden(true);
        self.input_separator.set_hidden(true);
        self.reply_bar.set_hidden(true);
        self.attachment_preview_bar.set_hidden(true);
        self.attachment_clear_button.set_hidden(true);
        self.input_bar.set_hidden(true);

        self.search_field.set_placeholder_text("Search");

        apply_visual_effect_sidebar(&self.sidebar);

        self.sidebar.add_subview(&self.search_field);
        self.sidebar.add_subview(&self.channel_list);

        LayoutConstraint::activate(&[
            self.search_field
                .top
                .constraint_equal_to(&self.sidebar.top)
                .offset(6.),
            self.search_field
                .leading
                .constraint_equal_to(&self.sidebar.leading)
                .offset(8.),
            self.search_field
                .trailing
                .constraint_equal_to(&self.sidebar.trailing)
                .offset(-8.),
            self.search_field.height.constraint_equal_to_constant(24.),
            self.channel_list
                .top
                .constraint_equal_to(&self.search_field.bottom)
                .offset(4.),
            self.channel_list
                .leading
                .constraint_equal_to(&self.sidebar.leading),
            self.channel_list
                .trailing
                .constraint_equal_to(&self.sidebar.trailing),
            self.channel_list
                .bottom
                .constraint_equal_to(&self.sidebar.bottom),
        ]);

        apply_separator_color(&self.sidebar_divider);

        self.main_view.add_subview(&self.sidebar);
        self.main_view.add_subview(&self.sidebar_divider);
        self.main_view.add_subview(&self.detail);

        LayoutConstraint::activate(&[
            self.sidebar
                .leading
                .constraint_equal_to(&self.main_view.leading),
            self.sidebar.top.constraint_equal_to(&self.main_view.top),
            self.sidebar
                .bottom
                .constraint_equal_to(&self.main_view.bottom),
            self.sidebar.width.constraint_equal_to_constant(250.0),
            // 1pt divider between sidebar and detail
            self.sidebar_divider
                .leading
                .constraint_equal_to(&self.sidebar.trailing),
            self.sidebar_divider
                .top
                .constraint_equal_to(&self.main_view.top),
            self.sidebar_divider
                .bottom
                .constraint_equal_to(&self.main_view.bottom),
            self.sidebar_divider.width.constraint_equal_to_constant(1.0),
            self.detail
                .leading
                .constraint_equal_to(&self.sidebar_divider.trailing),
            self.detail
                .trailing
                .constraint_equal_to(&self.main_view.trailing),
            self.detail.top.constraint_equal_to(&self.main_view.top),
            self.detail
                .bottom
                .constraint_equal_to(&self.main_view.bottom),
        ]);

        // --- Error view ---
        self.error_label.set_text("An error occurred");
        self.error_label
            .set_line_break_mode(cacao::text::LineBreakMode::WrapWords);
        self.error_retry_button.set_action(|_| {
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::RetrySetup);
        });
        self.error_view.add_subview(&self.error_label);
        self.error_view.add_subview(&self.error_retry_button);
        LayoutConstraint::activate(&[
            self.error_label
                .center_x
                .constraint_equal_to(&self.error_view.center_x),
            self.error_label
                .center_y
                .constraint_equal_to(&self.error_view.center_y)
                .offset(-20.),
            self.error_label
                .leading
                .constraint_equal_to(&self.error_view.leading)
                .offset(40.),
            self.error_label
                .trailing
                .constraint_equal_to(&self.error_view.trailing)
                .offset(-40.),
            self.error_retry_button
                .center_x
                .constraint_equal_to(&self.error_view.center_x),
            self.error_retry_button
                .top
                .constraint_equal_to(&self.error_label.bottom)
                .offset(16.),
        ]);

        // --- Add all views to content ---
        self.content.add_subview(&self.setup_view);
        self.content.add_subview(&self.qr_view);
        self.content.add_subview(&self.confirm_view);
        self.content.add_subview(&self.main_view);
        self.content.add_subview(&self.error_view);

        for view in [
            &self.setup_view,
            &self.qr_view,
            &self.confirm_view,
            &self.main_view,
            &self.error_view,
        ] {
            LayoutConstraint::activate(&[
                view.leading.constraint_equal_to(&self.content.leading),
                view.trailing.constraint_equal_to(&self.content.trailing),
                view.top.constraint_equal_to(&self.content.top),
                view.bottom.constraint_equal_to(&self.content.bottom),
            ]);
        }

        self.setup_label.set_text("Connecting...");
        self.link_button.set_hidden(true);
        self.device_name_field.set_hidden(true);
        self.show_only(&self.setup_view);

        window.set_content_view(&self.content);
    }
}

pub struct FlareWindow(Option<Window<FlareWindowDelegate>>);

impl FlareWindow {
    pub fn show(&self) {
        if let Some(ref w) = self.0 {
            // Persist and restore window size/position automatically.
            unsafe {
                use objc::{class, msg_send, sel, sel_impl};
                use std::ffi::CString;
                let win = &*w.objc as *const _ as *mut objc::runtime::Object;
                let name_cs = CString::new("FlareMainWindow").unwrap_or_default();
                let ns_str: *mut objc::runtime::Object = {
                    let s: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
                    msg_send![s, initWithUTF8String: name_cs.as_ptr()]
                };
                let _: () = msg_send![win, setFrameAutosaveName: ns_str];
            }
            w.show();
        }
    }

    pub fn start_backend(&self) {
        if let Some(ref w) = self.0 {
            let state = w.delegate.as_ref().unwrap().backend_state.clone();
            backend::start_backend(state);
        }
    }

    pub fn show_setup_pending(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            d.setup_label.set_text("Welcome to Flare");
            d.link_button.set_hidden(false);
            d.device_name_field.set_hidden(false);
            d.show_only(&d.setup_view);
        }
    }

    pub fn show_qr_code(&self, png_data: &[u8]) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            let image = Image::with_data(png_data);
            d.qr_image_view.set_image(&image);
            d.show_only(&d.qr_view);
        }
    }

    pub fn show_confirmation_entry(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            d.show_only(&d.confirm_view);
        }
    }

    pub fn show_main_view(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            d.show_only(&d.main_view);
        }
    }

    pub fn show_error(&self, msg: &str) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            d.error_label.set_text(msg);
            d.show_only(&d.error_view);
        }
    }

    pub fn retry_setup(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            d.setup_label.set_text("Connecting...");
            d.link_button.set_hidden(true);
            d.device_name_field.set_hidden(true);
            d.show_only(&d.setup_view);
            let state = d.backend_state.clone();
            backend::start_backend(state);
        }
    }

    pub fn set_channels(&self, channels: Vec<CoreChannel>) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            if let Some(ref delegate) = d.channel_list.delegate {
                delegate.set_channels(channels);
                // Keep contact picker in sync so it always shows latest contacts.
                d.contact_picker.update_contacts(delegate.all_channels());
            }
        }
    }

    pub fn filter_channels(&self, query: &str) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            if let Some(ref delegate) = d.channel_list.delegate {
                delegate.set_filter(query);
            }
        }
    }

    pub fn set_messages(&self, channel_id: ChannelId, messages: Vec<CoreMessage>) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();

            // Save draft for the channel we're leaving
            if let Some(ref old_id) = d.current_channel.borrow().clone() {
                let draft = d.input_field.get_value();
                if draft.trim().is_empty() {
                    d.drafts.borrow_mut().remove(old_id);
                } else {
                    d.drafts.borrow_mut().insert(old_id.clone(), draft);
                }
            }

            if let Some(ref list_delegate) = d.channel_list.delegate {
                let channel_data = list_delegate.find_channel(&channel_id);
                if let Some(ref ch) = channel_data {
                    d.conversation_header.set_text(&ch.title);
                }
                *d.current_channel_data.borrow_mut() = channel_data;
                list_delegate.clear_unread(&channel_id);
                list_delegate.set_active_channel(&channel_id);
            }
            *d.current_channel.borrow_mut() = Some(channel_id.clone());
            let count = messages.len();
            *d.loaded_message_count.borrow_mut() = count;

            d.conversation_header.set_hidden(false);
            d.detail_header_sep.set_hidden(false);
            d.detail_placeholder.set_hidden(true);
            let show_load_more = count >= 50;
            set_height_constraint(&d.load_more_bar, if show_load_more { 28.0 } else { 0.0 });
            set_view_alpha(&d.load_more_bar, 0.15);
            d.message_list.set_hidden(false);
            d.typing_label.set_hidden(true);
            set_height_constraint(&d.typing_label, 0.0);
            d.input_separator.set_hidden(false);
            d.reply_bar.set_hidden(true);
            set_height_constraint(&d.reply_bar, 0.0);
            *d.current_reply.borrow_mut() = None;
            d.input_bar.set_hidden(false);

            // Restore draft for this channel
            let draft = d
                .drafts
                .borrow()
                .get(&channel_id)
                .cloned()
                .unwrap_or_default();
            d.input_field.set_text(&draft);

            if let Some(ref delegate) = d.message_list.delegate {
                delegate.set_messages(messages);
            }
        }
    }

    pub fn prepend_messages(&self, messages: Vec<CoreMessage>) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            let added = messages.len();
            let show_load_more = added >= 50;
            set_height_constraint(&d.load_more_bar, if show_load_more { 28.0 } else { 0.0 });
            set_view_alpha(&d.load_more_bar, 0.15);
            *d.loaded_message_count.borrow_mut() += added;
            if let Some(ref delegate) = d.message_list.delegate {
                delegate.prepend_messages(messages);
            }
        }
    }

    pub fn add_message(&self, message: CoreMessage) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            let current = d.current_channel.borrow().clone();
            if message.channel_id.as_ref() != current.as_ref() {
                // Message is for a different (or no) channel — increment unread badge
                if let Some(ref msg_cid) = message.channel_id {
                    if let Some(ref list_delegate) = d.channel_list.delegate {
                        list_delegate.increment_unread(msg_cid);
                    }
                }
                return;
            }
            if current.is_none() {
                return;
            }
            d.typing_label.set_hidden(true);
            set_height_constraint(&d.typing_label, 0.0);
            d.typing_label.set_text("");
            if !message.is_reaction {
                *d.loaded_message_count.borrow_mut() += 1;
            }
            if let Some(ref delegate) = d.message_list.delegate {
                delegate.add_message(message);
            }
        }
    }

    pub fn update_channel_last_message(
        &self,
        channel_id: ChannelId,
        body: Option<String>,
        timestamp: u64,
    ) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            if let Some(ref list_delegate) = d.channel_list.delegate {
                list_delegate.update_channel_last_message(&channel_id, body, timestamp);
            }
        }
    }

    pub fn show_typing(&self, channel_id: ChannelId) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            let current = d.current_channel.borrow().clone();
            if current.as_ref() == Some(&channel_id) {
                d.typing_label.set_text("typing...");
                d.typing_label.set_hidden(false);
                set_height_constraint(&d.typing_label, 18.0);
                // Auto-clear after 5 seconds
                let cid = channel_id.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    App::<FlareApp, AppMessage>::dispatch_main(AppMessage::TypingCleared(cid));
                });
            }
        }
    }

    pub fn clear_typing(&self, channel_id: ChannelId) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            let current = d.current_channel.borrow().clone();
            if current.as_ref() == Some(&channel_id) {
                d.typing_label.set_hidden(true);
                set_height_constraint(&d.typing_label, 0.0);
                d.typing_label.set_text("");
            }
        }
    }

    pub fn handle_load_more(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            let channel_id = d.current_channel.borrow().clone();
            if let Some(channel_id) = channel_id {
                let offset = *d.loaded_message_count.borrow();
                if let Some(tx) = d.backend_state.command_tx.lock().unwrap().as_ref() {
                    let _ =
                        tx.unbounded_send(BackendCommand::LoadOlderMessages(channel_id, offset));
                }
                set_height_constraint(&d.load_more_bar, 0.0);
            }
        }
    }

    pub fn selected_message_body(&self) -> Option<String> {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            if let Some(ref delegate) = d.message_list.delegate {
                return delegate.selected_message_body();
            }
        }
        None
    }

    pub fn selected_message(&self) -> Option<CoreMessage> {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            if let Some(ref delegate) = d.message_list.delegate {
                return delegate.selected_message();
            }
        }
        None
    }

    pub fn set_reply(&self, message: CoreMessage) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            let preview = message.body.as_deref().unwrap_or("[attachment]");
            let truncated: String = preview.chars().take(60).collect();
            d.reply_label.set_text(&format!("↩ {}", truncated));
            *d.current_reply.borrow_mut() = Some(message);
            d.reply_bar.set_hidden(false);
            set_height_constraint(&d.reply_bar, 36.0);
        }
    }

    pub fn clear_reply(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            *d.current_reply.borrow_mut() = None;
            d.reply_bar.set_hidden(true);
            set_height_constraint(&d.reply_bar, 0.0);
        }
    }

    pub fn current_channel_info(&self) -> Option<CoreChannel> {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            return d.current_channel_data.borrow().clone();
        }
        None
    }

    pub fn filter_contact_picker(&self, query: &str) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            d.contact_picker.filter_contacts(query);
        }
    }

    pub fn open_contact_picker(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            let contacts = if let Some(ref list_delegate) = d.channel_list.delegate {
                list_delegate.all_channels()
            } else {
                vec![]
            };
            d.contact_picker.show_with_contacts(contacts);
        }
    }

    pub fn fetch_devices(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            if let Some(tx) = d.backend_state.command_tx.lock().unwrap().as_ref() {
                let _ = tx.unbounded_send(BackendCommand::FetchDevices);
            }
        }
    }

    pub fn send_command(&self, cmd: super::backend::BackendCommand) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            if let Some(tx) = d.backend_state.command_tx.lock().unwrap().as_ref() {
                let _ = tx.unbounded_send(cmd);
            }
        }
    }

    pub fn clear_current_channel_messages(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            let channel_id = d.current_channel.borrow().clone();
            if let Some(channel_id) = channel_id {
                if super::alert::confirm(
                    "Clear Conversation",
                    "This will remove all locally stored messages from this conversation.",
                    "Clear",
                    "Cancel",
                ) {
                    if let Some(tx) = d.backend_state.command_tx.lock().unwrap().as_ref() {
                        let _ = tx.unbounded_send(BackendCommand::ClearChannelMessages(channel_id));
                    }
                }
            }
        }
    }

    pub fn handle_attach(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            if let Some(_channel_id) = d.current_channel.borrow().clone() {
                let paths = super::alert::pick_files();
                for path in paths {
                    d.add_pending_attachment(path);
                }
            }
        }
    }

    pub fn handle_paste_file(&self, path: String) {
        self.handle_paste_files(vec![path]);
    }

    pub fn handle_paste_files(&self, paths: Vec<String>) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            if let Some(_channel_id) = d.current_channel.borrow().clone() {
                for path in paths {
                    let path = std::path::PathBuf::from(path);
                    d.add_pending_attachment(path);
                }
            }
        }
    }

    pub fn handle_paste_image(&self, data: Vec<u8>, filename: String) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            if let Some(_channel_id) = d.current_channel.borrow().clone() {
                let temp_dir = std::env::temp_dir();
                let path = temp_dir.join(&filename);
                if let Err(e) = std::fs::write(&path, &data) {
                    log::error!("Failed to write pasted image to temp file: {}", e);
                    return;
                }
                d.add_pending_attachment(path);
            }
        }
    }

    pub fn handle_clear_attachments(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            d.clear_attachment_preview_bar();
        }
    }

    pub fn handle_react(&self, emoji: &str) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            let channel_id = d.current_channel.borrow().clone();
            if let Some(channel_id) = channel_id {
                if let Some(ref msg_delegate) = d.message_list.delegate {
                    if let Some(msg) = msg_delegate.selected_message() {
                        if let Some(tx) = d.backend_state.command_tx.lock().unwrap().as_ref() {
                            let _ =
                                tx.unbounded_send(super::backend::BackendCommand::SendReaction(
                                    channel_id.clone(),
                                    msg.timestamp,
                                    msg.sender,
                                    emoji.to_string(),
                                ));
                        }
                    }
                }
            }
        }
    }

    pub fn handle_send(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            let text = d.input_field.get_value();
            let text = text.trim().to_string();
            let channel_id = d.current_channel.borrow().clone();
            if let Some(channel_id) = channel_id {
                // Send pending attachments first (they clear the attachment bar)
                d.send_pending_attachments();

                // Then send text message if there's text
                if !text.is_empty() {
                    let reply = d.current_reply.borrow_mut().take();
                    let quote = reply.map(|m| crate::core::message::QuoteData {
                        ts: m.timestamp,
                        text: m.body.clone(),
                    });
                    if let Some(tx) = d.backend_state.command_tx.lock().unwrap().as_ref() {
                        let _ = tx.unbounded_send(BackendCommand::SendMessage(
                            channel_id.clone(),
                            text,
                            quote,
                        ));
                    }
                    d.reply_bar.set_hidden(true);
                    set_height_constraint(&d.reply_bar, 0.0);
                    d.drafts.borrow_mut().remove(&channel_id);
                    d.input_field.set_text("");
                }
            }
        }
    }

    pub fn handle_receipts(&self, timestamps: &[u64], is_read: bool) {
        super::message_view::update_receipt_status(timestamps, is_read);
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            if let Some(ref delegate) = d.message_list.delegate {
                if let Some(ref view) = delegate.view {
                    super::message_view::reload_listview(view);
                }
            }
        }
    }

    pub fn is_active_channel(&self, channel_id: &ChannelId) -> bool {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            return d.current_channel.borrow().as_ref() == Some(channel_id);
        }
        false
    }

    pub fn activate_channel_at_index(&self, index: usize) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            if let Some(ref delegate) = d.channel_list.delegate {
                delegate.activate_channel_at_index(index);
            }
        }
    }

    pub fn focus_input(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            d.input_field.objc.get(|obj| unsafe {
                use objc::{msg_send, sel, sel_impl};
                let obj = obj as *const _ as *mut objc::runtime::Object;
                let win: *mut objc::runtime::Object = msg_send![obj, window];
                if !win.is_null() {
                    let _: bool = msg_send![win, makeFirstResponder: obj];
                }
            });
        }
    }

    pub fn focus_search(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            d.search_field.objc.get(|obj| unsafe {
                use objc::{msg_send, sel, sel_impl};
                let obj = obj as *const _ as *mut objc::runtime::Object;
                let win: *mut objc::runtime::Object = msg_send![obj, window];
                if !win.is_null() {
                    let _: bool = msg_send![win, makeFirstResponder: obj];
                }
            });
        }
    }

    pub fn delete_selected_message(&self) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            let channel_id = d.current_channel.borrow().clone();
            if let Some(channel_id) = channel_id {
                if let Some(ref delegate) = d.message_list.delegate {
                    if let Some(msg) = delegate.selected_message() {
                        if let Some(tx) = d.backend_state.command_tx.lock().unwrap().as_ref() {
                            let _ = tx.unbounded_send(BackendCommand::DeleteMessage(
                                channel_id,
                                msg.timestamp,
                            ));
                        }
                    }
                }
            }
        }
    }

    pub fn remove_message(&self, timestamp: u64) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            if let Some(ref delegate) = d.message_list.delegate {
                delegate.remove_message(timestamp);
            }
        }
    }

    pub fn reload_channel(&self, channel_id: ChannelId) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            let current = d.current_channel.borrow().clone();
            if current == Some(channel_id.clone()) {
                // Reload messages for this channel
                if let Some(tx) = d.backend_state.command_tx.lock().unwrap().as_ref() {
                    let _ = tx.unbounded_send(BackendCommand::LoadMessages(channel_id));
                }
            }
        }
    }
}

impl Default for FlareWindow {
    fn default() -> Self {
        let mut config = WindowConfig::default();
        config.set_styles(&[
            WindowStyle::Titled,
            WindowStyle::Closable,
            WindowStyle::Miniaturizable,
            WindowStyle::Resizable,
            WindowStyle::UnifiedTitleAndToolbar,
        ]);
        config.initial_dimensions = Rect::new(0.0, 0.0, 900.0, 600.0);

        Self(Some(Window::with(config, FlareWindowDelegate::new())))
    }
}
