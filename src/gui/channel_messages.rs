use gdk::subclass::prelude::ObjectSubclassIsExt;
use glib::ObjectExt;
use gtk::traits::WidgetExt;
use gtk::{gdk, glib};

use crate::backend::{message::TextMessage, Channel, Manager};

glib::wrapper! {
    pub struct ChannelMessages(ObjectSubclass<imp::ChannelMessages>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl ChannelMessages {
    pub fn focus_input(&self) {
        self.imp().text_entry.grab_focus();
    }

    pub fn load_more(&self) {
        self.imp().handle_more();
    }

    pub fn manager(&self) -> Manager {
        self.property("manager")
    }

    pub fn reply_message(&self) -> Option<TextMessage> {
        self.property("reply-message")
    }

    pub fn set_reply_message(&self, msg: &Option<TextMessage>) {
        self.set_property("reply-message", msg)
    }

    pub fn active_channel(&self) -> Option<Channel> {
        self.property("active-channel")
    }
}

pub mod imp {

    // At least 4 minutes need to pass that for two messages from the same sender, the second one will
    // also show avatar and sender title.
    const MESSAGE_SENT_SHOW_NAME_DURATION: u64 = 4 * 60 * 1000;

    use std::{cell::RefCell, time::Duration};

    use gio::Settings;
    use glib::{
        clone, once_cell::sync::Lazy, subclass::InitializingObject, ParamSpec, ParamSpecBoolean,
        ParamSpecObject, SignalHandlerId, Value,
    };
    use gtk::{gio, glib, FileChooserNative};
    use gtk::{
        prelude::*, subclass::prelude::*, CompositeTemplate, FileChooserAction, ResponseType,
    };

    use crate::{
        backend::{
            message::{CallMessage, DisplayMessage, MessageExt, TextMessage},
            Channel, Manager,
        },
        config::APP_ID,
        gspawn,
        gui::{
            attachment::Attachment, call_message_item::CallMessageItem, error_dialog::ErrorDialog,
            message_item::MessageItem, text_entry::TextEntry, utility::Utility,
        },
    };

    #[derive(CompositeTemplate)]
    #[template(resource = "/ui/channel_messages.ui")]
    pub struct ChannelMessages {
        #[template_child]
        pub(super) list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub(super) scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        box_attachments: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) text_entry: TemplateChild<TextEntry>,

        attachments: RefCell<Vec<crate::backend::Attachment>>,
        reply_message: RefCell<Option<TextMessage>>,

        manager: RefCell<Option<Manager>>,
        active_channel: RefCell<Option<Channel>>,
        last_signal_handler: RefCell<Option<SignalHandlerId>>,

        pub(super) settings: Settings,
    }

    impl Default for ChannelMessages {
        fn default() -> Self {
            Self {
                list: Default::default(),
                scrolled_window: Default::default(),
                box_attachments: Default::default(),
                text_entry: Default::default(),
                attachments: Default::default(),
                reply_message: Default::default(),
                manager: Default::default(),
                active_channel: Default::default(),
                last_signal_handler: Default::default(),
                settings: Settings::new(APP_ID),
            }
        }
    }

    #[gtk::template_callbacks]
    impl ChannelMessages {
        #[template_callback]
        pub(super) fn handle_more(&self) {
            log::trace!("More messages were requested in the UI");
            let channel = self.active_channel.borrow();
            if let Some(channel) = channel.as_ref() {
                let obj = self.obj();
                let to_load = self.settings.int("messages-request-load");
                gspawn!(glib::clone!(@strong channel, @strong obj => async move {
                    let mut msgs = channel.load_last(to_load.try_into().unwrap_or(1)).await;
                    msgs.reverse();
                    for msg in msgs {
                        obj.imp().prepend_message(&msg);
                    }
                }));
            } else {
                log::warn!("More messages were requested while not being focused on a channel. This should not happen.");
            }
        }

        #[template_callback]
        fn remove_reply(&self) {
            log::trace!("Unsetting reply message");
            self.obj().set_reply_message(&None);
        }

        #[template_callback]
        fn remove_attachments(&self) {
            log::trace!("Unsetting attachments");
            {
                let mut att = self.attachments.borrow_mut();
                att.clear();
                while let Some(child) = self.box_attachments.first_child() {
                    self.box_attachments.remove(&child);
                }
            }
            self.obj().notify("has-attachments");
        }

        fn append_attachment(&self, attachment: crate::backend::Attachment) {
            let att_widget = Attachment::new(&attachment);
            self.box_attachments.append(&att_widget);
            self.attachments.borrow_mut().push(attachment);
        }

        #[template_callback]
        fn paste_file(&self, file: gio::File) {
            log::trace!("`ChannelMessages` got file as attachment.");
            let obj = self.obj();
            let manager = obj.manager();
            let attachment = crate::backend::Attachment::from_file(file, &manager);
            self.append_attachment(attachment);
            obj.notify("has-attachments");
        }

        #[template_callback]
        fn paste_texture(&self, texture: gdk::Texture) {
            log::trace!("`ChannelMessages` got texture as attachment.");
            let obj = self.obj();
            let manager = obj.manager();
            let attachment = crate::backend::Attachment::from_texture(texture, &manager);
            self.append_attachment(attachment);
            obj.notify("has-attachments");
        }

        #[template_callback]
        fn add_attachment(&self) {
            log::trace!("Requested to add a attachment");
            let chooser = FileChooserNative::builder()
                .transient_for(
                    &self
                        .obj()
                        .root()
                        .expect("`ChannelMessages` to have a root")
                        .dynamic_cast::<crate::gui::Window>()
                        .expect("Root of `ChannelMessages` to be a `Window`."),
                )
                .action(FileChooserAction::Open)
                .build();
            let obj = self.obj();
            chooser.connect_response(clone!(@strong chooser, @strong obj => move |_, action| {
                if action == ResponseType::Accept {
                    log::trace!("User added an attachment");
                    let file = chooser.file();
                    if let Some(file) = file {
                        obj.imp().paste_file(file);
                    }
                } else {
                    log::trace!("User did not upload a attachment");
                }
            }));
            chooser.show();
        }

        #[template_callback]
        fn send_message(&self) {
            log::trace!("Got callback to send message");
            let text = self.text_entry.text();
            self.text_entry.clear();
            let attachments = {
                let mut att = self.attachments.borrow_mut();
                let a = att.clone();
                att.clear();
                a
            };
            self.obj().notify("has-attachments");

            if text.is_empty() && attachments.is_empty() {
                log::warn!("Got requested to send empty message, skipping");
            }

            while let Some(child) = self.box_attachments.first_child() {
                self.box_attachments.remove(&child);
            }

            let obj = self.obj();
            if let Some(channel) = self.active_channel.borrow().as_ref() {
                log::trace!("Constructing message");
                let manager = self.obj().manager();

                let msg = TextMessage::from_text_channel_sender(
                    text,
                    channel.clone(),
                    manager.self_contact(),
                    &manager,
                );

                if let Some(quote) = obj.reply_message() {
                    log::trace!("Adding quote to message");
                    msg.set_quote(&quote);
                    obj.set_reply_message(&None);
                }

                let obj = self.obj();
                gspawn!(
                    clone!(@strong msg, @strong channel, @strong attachments, @strong obj => async move {
                        log::trace!("Adding attachments to message: {}", attachments.len());
                        for att in attachments {
                            if let Err(e) = msg.add_attachment(att).await {
                                let root = obj
                                    .root()
                                    .expect("`ChannelMessages` to have a root")
                                    .dynamic_cast::<crate::gui::Window>()
                                    .expect("Root of `ChannelMessages` to be a `Window`.");
                                let dialog = ErrorDialog::new(e, &root);
                                dialog.show();
                                return;
                            }
                        }
                        log::trace!("Sending message");
                        if let Err(e) = channel.send_message(msg.upcast()).await {
                            let root = obj
                                .root()
                                .expect("`ChannelMessages` to have a root")
                                .dynamic_cast::<crate::gui::Window>()
                                .expect("Root of `ChannelMessages` to be a `Window`.");
                            let dialog = ErrorDialog::new(e, &root);
                            dialog.show();
                        }
                    })
                );
            }
        }

        #[template_callback]
        fn handle_row_activated(&self, row: gtk::ListBoxRow) {
            if let Ok(msg) = row
                .child()
                .expect("`ListBoxRow` to have a child")
                .dynamic_cast::<MessageItem>()
            {
                crate::trace!(
                    "Activated message: {}",
                    msg.message().body().unwrap_or_default()
                );
                msg.open_popup();
            }
        }
    }

    impl ChannelMessages {
        fn reset_messages(&self) {
            self.obj().set_reply_message(&None);
            while let Some(child) = self.list.first_child() {
                self.list.remove(&child);
            }
        }

        fn add_message(&self, message: &DisplayMessage) {
            let obj = self.obj();
            if let Some(message) = message.dynamic_cast_ref::<TextMessage>() {
                let widget = MessageItem::new(message);
                self.list.append(&widget);
                self.update_show_name_of(&widget);
                widget.connect_local(
                    "reply",
                    false,
                    clone!(@strong obj => move |args| {
                        let msg = args[1]
                            .get::<TextMessage>()
                            .expect("Type of signal `reply` of `MessageItem` to be `TextMessage`.");
                        obj.set_reply_message(&Some(msg));
                        None
                    }),
                );
            } else if let Some(message) = message.dynamic_cast_ref::<CallMessage>() {
                let widget = CallMessageItem::new(message);
                self.list.append(&widget);
            } else {
                log::warn!("`ChannelMessages` was asked to display an unknown `DisplayMessage`");
            }

            // Scroll to bottom
            gspawn!(clone!(@strong obj => async move  {
                // Need to sleep a little to make sure the scrolled window saw the changed
                // child.
                glib::timeout_future(Duration::from_millis(50)).await;
                let adjustment = obj.imp().scrolled_window.vadjustment();
                adjustment.set_value(adjustment.upper());
            }));
        }

        fn update_show_name_of(&self, widget: &MessageItem) {
            let obj = self.obj();
            let message: DisplayMessage = widget.message().upcast();
            let message_sender_title = message.sender().title();
            let last_message = obj
                .active_channel()
                .and_then(|c| c.previous_message_to(&message));
            let last_message_sender_title = last_message.as_ref().map(|m| m.sender().title());
            let sent = message.sent();
            let last_message_sent = last_message.map(|m| m.sent()).unwrap_or_default();
            widget.set_property(
                "show-name",
                last_message_sender_title != Some(message_sender_title)
                    || sent > last_message_sent + MESSAGE_SENT_SHOW_NAME_DURATION,
            );
        }

        fn prepend_message(&self, message: &DisplayMessage) {
            let obj = self.obj();
            if let Some(message) = message.dynamic_cast_ref::<TextMessage>() {
                let widget = MessageItem::new(message);
                self.list.insert(&widget, 0);
                self.update_show_name_of(&widget);
                widget.connect_local(
                    "reply",
                    false,
                    clone!(@strong obj => move |args| {
                        let msg = args[1]
                            .get::<TextMessage>()
                            .expect("Type of signal `reply` of `MessageItem` to be `TextMessage`.");
                        obj.set_reply_message(&Some(msg));
                        None
                    }),
                );
            } else if let Some(message) = message.dynamic_cast_ref::<CallMessage>() {
                let widget = CallMessageItem::new(message);
                self.list.append(&widget);
            } else {
                log::warn!("`ChannelMessages` was asked to display an unknown `DisplayMessage`");
            }

            if let Some(previous_first) = self.list.row_at_index(1) {
                if let Some(previous_first) = previous_first
                    .child()
                    .expect("Message list row to have a child")
                    .dynamic_cast_ref::<MessageItem>()
                {
                    self.update_show_name_of(previous_first);
                }
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChannelMessages {
        const NAME: &'static str = "FlChannelMessages";
        type Type = super::ChannelMessages;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
            Utility::bind_template_callbacks(klass);
            MessageItem::ensure_type();
            TextEntry::ensure_type();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ChannelMessages {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().connect_notify_local(
                Some("active-channel"),
                clone!(@weak self as obj => move |_, _| {
                    obj.obj().set_reply_message(&None);
                }),
            );
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<Manager>("manager").build(),
                    ParamSpecObject::builder::<Channel>("active-channel").build(),
                    ParamSpecObject::builder::<TextMessage>("reply-message").build(),
                    ParamSpecBoolean::builder("has-attachments").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "active-channel" => self.active_channel.borrow().as_ref().to_value(),
                "reply-message" => self.reply_message.borrow().as_ref().to_value(),
                "has-attachments" => (!self.attachments.borrow().is_empty()).to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let man = value.get::<Option<Manager>>().expect(
                        "Property `manager` of `ChannelMessages` has to be of type `Manager`",
                    );
                    self.manager.replace(man);
                }
                "active-channel" => {
                    let chan = value.get::<Option<Channel>>().expect(
                        "Property `active-channel` of `ChannelMessages` has to be of type `Channel`",
                    );
                    self.reset_messages();
                    let previous_channel = self.active_channel.replace(chan.clone());
                    if let Some(channel) = &chan {
                        for msg in channel.messages() {
                            self.add_message(&msg);
                        }

                        let mut signal_handler = self.last_signal_handler.borrow_mut();
                        if let Some(sig) = signal_handler.take() {
                            glib::signal::signal_handler_disconnect(
                                previous_channel
                                    .as_ref()
                                    .expect("A `active-channel` of `ChannelMessages`"),
                                sig,
                            );
                        }
                        signal_handler.replace(
                                channel.connect_local("message", false, clone!(@weak self as obj  => @default-return None, move |args| {
                                    let msg = args[1].get::<DisplayMessage>().expect("Type of signal `message` of `Channel` to be `DisplayMessage`");
                                    obj.add_message(&msg);
                                    None
                                }))
                        );
                    }
                }
                "reply-message" => {
                    let msg = value.get::<Option<TextMessage>>().expect(
                        "Property `reply-message` of `ChannelMessages` has to be of type `TextMessage`",
                    );
                    self.reply_message.replace(msg);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for ChannelMessages {}
    impl BoxImpl for ChannelMessages {}
}
