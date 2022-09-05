use gdk::subclass::prelude::ObjectSubclassIsExt;
use gtk::traits::WidgetExt;

gtk::glib::wrapper! {
    pub struct ChannelMessages(ObjectSubclass<imp::ChannelMessages>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl ChannelMessages {
    pub fn focus_input(&self) {
        self.imp().text_entry.grab_focus();
    }
}

pub mod imp {
    use std::cell::RefCell;

    use gdk_pixbuf::glib::clone;
    use gdk_pixbuf::glib::once_cell::sync::Lazy;
    use gdk_pixbuf::glib::MainContext;
    use gdk_pixbuf::glib::ParamFlags;
    use gdk_pixbuf::glib::ParamSpec;
    use gdk_pixbuf::glib::ParamSpecBoolean;
    use gdk_pixbuf::glib::ParamSpecObject;
    use gdk_pixbuf::glib::SignalHandlerId;
    use gdk_pixbuf::glib::Value;
    use glib::subclass::InitializingObject;
    use gtk::builders::FileChooserNativeBuilder;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::CompositeTemplate;
    use gtk::FileChooserAction;
    use gtk::ResponseType;

    use crate::backend::Channel;
    use crate::backend::Contact;
    use crate::backend::Manager;
    use crate::backend::Message;
    use crate::gui::attachment::Attachment;
    use crate::gui::error_dialog::ErrorDialog;
    use crate::gui::message_item::MessageItem;
    use crate::gui::text_entry::TextEntry;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/channel_messages.ui")]
    pub struct ChannelMessages {
        #[template_child]
        pub(super) list: TemplateChild<gtk::ListBox>,
        #[template_child]
        box_attachments: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) text_entry: TemplateChild<TextEntry>,

        attachments: RefCell<Vec<crate::backend::Attachment>>,
        reply_message: RefCell<Option<Message>>,

        manager: RefCell<Option<Manager>>,
        active_channel: RefCell<Option<Channel>>,
        last_signal_handler: RefCell<Option<SignalHandlerId>>,
    }

    #[gtk::template_callbacks]
    impl ChannelMessages {
        #[template_callback]
        fn remove_reply(&self) {
            log::trace!("Unsetting reply message");
            self.instance()
                .set_property("reply_message", None::<Message>);
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
            self.instance().notify("has-attachments");
        }

        fn append_attachment(&self, attachment: crate::backend::Attachment) {
            let att_widget = Attachment::new(&attachment);
            self.box_attachments.append(&att_widget);
            self.attachments.borrow_mut().push(attachment);
        }

        #[template_callback]
        fn add_attachment(&self) {
            log::trace!("Requested to add a attachment");
            let chooser = FileChooserNativeBuilder::new()
                .transient_for(
                    &self
                        .instance()
                        .root()
                        .expect("`ChannelMessages` to have a root")
                        .dynamic_cast::<crate::gui::Window>()
                        .expect("Root of `ChannelMessages` to be a `Window`."),
                )
                .action(FileChooserAction::Open)
                .build();
            let manager = self.instance().property::<Manager>("manager");
            let obj = self.instance();
            chooser.connect_response(
                clone!(@strong chooser, @strong obj, @strong manager => move |_, action| {
                    if action == ResponseType::Accept {
                        log::trace!("User added an attachment");
                        let file = chooser.file();
                        if let Some(file) = file {
                            let attachment = crate::backend::Attachment::from_file(file, &manager);
                            obj.imp().append_attachment(attachment);
                            obj.notify("has-attachments");
                        }
                    } else {
                        log::trace!("User did not upload a attachment");
                    }
                }),
            );
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
            while let Some(child) = self.box_attachments.first_child() {
                self.box_attachments.remove(&child);
            }

            let obj = self.instance();
            if let Some(channel) = self.active_channel.borrow().as_ref() {
                log::trace!("Constructing message");
                let manager = self.instance().property::<Manager>("manager");

                let msg = Message::from_text_channel_sender(
                    text,
                    channel.clone(),
                    manager.self_contact(),
                    &manager,
                );

                if let Some(quote) = obj.property::<Option<Message>>("reply-message") {
                    log::trace!("Adding quote to message");
                    msg.set_quote(quote);
                    obj.set_property("reply-message", &None::<Message>);
                }

                let main_context = MainContext::default();
                let obj = self.instance();
                main_context.spawn_local(
                    clone!(@strong msg, @strong channel, @strong attachments, @strong obj => async move {
                        log::trace!("Adding attachments to message: {}", attachments.len());
                        for att in attachments {
                            if let Err(e) = msg.add_attachment(att).await {
                                let root = obj
                                    .root()
                                    .expect("`MessageItem` to have a root")
                                    .dynamic_cast::<crate::gui::Window>()
                                    .expect("Root of `ChannelMessages` to be a `Window`.");
                                let dialog = ErrorDialog::new(e, &root);
                                dialog.show();
                                return;
                            }
                        }
                        log::trace!("Sending message");
                        if let Err(e) = channel.send_message(msg).await {
                            let root = obj
                                .root()
                                .expect("`MessageItem` to have a root")
                                .dynamic_cast::<crate::gui::Window>()
                                .expect("Root of `ChannelMessages` to be a `Window`.");
                            let dialog = ErrorDialog::new(e, &root);
                            dialog.show();
                        }
                    }),
                );
            }
        }

        #[template_callback(function)]
        fn is_some(opt: Option<glib::Object>) -> bool {
            opt.is_some()
        }

        #[template_callback(function)]
        fn is_none(opt: Option<glib::Object>) -> bool {
            opt.is_none()
        }

        #[template_callback]
        fn handle_row_activated(&self, row: gtk::ListBoxRow) {
            let msg = row
                .child()
                .expect("`ListBoxRow` to have a child")
                .dynamic_cast::<MessageItem>()
                .expect("`ListBoxRow` to have a `MessageItem` child");
            crate::trace!(
                "Activated message: {}",
                msg.property::<Message>("message")
                    .property::<Option<String>>("body")
                    .unwrap_or_else(|| "".to_string())
            );
            msg.open_popup();
        }
    }

    impl ChannelMessages {
        fn reset_messages(&self) {
            self.instance()
                .set_property("reply-message", &None::<Message>);
            while let Some(child) = self.list.first_child() {
                self.list.remove(&child);
            }
        }

        fn add_message(&self, message: &Message) {
            let widget = MessageItem::new(message);
            self.list.append(&widget);
            let obj = self.instance();
            let message_sender_title = message
                .property::<Option<Contact>>("sender")
                .and_then(|s| s.property::<Option<String>>("title"));
            let last_message_sender_title = obj
                .property::<Option<Channel>>("active-channel")
                .and_then(|c| c.previous_message_to(message))
                .and_then(|m| m.property::<Option<Contact>>("sender"))
                .and_then(|s| s.property::<Option<String>>("title"));
            widget.set_property(
                "show-name",
                last_message_sender_title != message_sender_title,
            );
            widget.connect_local(
                "reply",
                false,
                clone!(@strong obj => move |args| {
                    let msg = args[1]
                        .get::<Message>()
                        .expect("Type of signal `reply` of `MessageItem` to be `Message`.");
                    obj.set_property("reply-message", &msg);
                    None
                }),
            );
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
            MessageItem::ensure_type();
            TextEntry::ensure_type();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ChannelMessages {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            obj.connect_notify_local(
                Some("active-channel"),
                clone!(@weak obj => move |_, _| {
                    obj.set_property("reply-message", &None::<Message>);
                }),
            );
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::new(
                        "manager",
                        "manager",
                        "manager",
                        Manager::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecObject::new(
                        "active-channel",
                        "active-channel",
                        "active-channel",
                        Channel::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecObject::new(
                        "reply-message",
                        "reply-message",
                        "reply-message",
                        Message::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecBoolean::new(
                        "has-attachments",
                        "has-attachments",
                        "has-attachments",
                        false,
                        ParamFlags::READABLE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "active-channel" => self.active_channel.borrow().as_ref().to_value(),
                "reply-message" => self.reply_message.borrow().as_ref().to_value(),
                "has-attachments" => (!self.attachments.borrow().is_empty()).to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
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
                    self.active_channel.replace(chan.clone());
                    if let Some(channel) = &chan {
                        for msg in channel.messages() {
                            self.add_message(&msg);
                        }

                        let mut signal_handler = self.last_signal_handler.borrow_mut();
                        if let Some(sig) = signal_handler.take() {
                            glib::signal::signal_handler_disconnect(
                                self.active_channel
                                    .borrow()
                                    .as_ref()
                                    .expect("A `active-channel` of `ChannelMessages`"),
                                sig,
                            );
                        }
                        signal_handler.replace(
                                channel.connect_local("message", false, clone!(@strong obj => move |args| {
                                    let msg = args[1].get::<Message>().expect("Type of signal `message` of `Channel` to be `Message`");
                                    obj.imp().add_message(&msg);
                                    None
                                }))
                        );
                    }
                }
                "reply-message" => {
                    let msg = value.get::<Option<Message>>().expect(
                        "Property `reply-message` of `ChannelMessages` has to be of type `Message`",
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
