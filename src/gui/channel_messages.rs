gtk::glib::wrapper! {
    pub struct ChannelMessages(ObjectSubclass<imp::ChannelMessages>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

pub mod imp {
    use std::cell::RefCell;

    use gdk_pixbuf::glib::clone;
    use gdk_pixbuf::glib::once_cell::sync::Lazy;
    use gdk_pixbuf::glib::MainContext;
    use gdk_pixbuf::glib::ParamFlags;
    use gdk_pixbuf::glib::ParamSpec;
    use gdk_pixbuf::glib::ParamSpecObject;
    use gdk_pixbuf::glib::SignalHandlerId;
    use gdk_pixbuf::glib::Value;
    use glib::subclass::InitializingObject;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::CompositeTemplate;

    use crate::backend::Channel;
    use crate::backend::Contact;
    use crate::backend::Manager;
    use crate::backend::Message;
    use crate::gui::message_item::MessageItem;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/channel_messages.ui")]
    pub struct ChannelMessages {
        #[template_child]
        pub(super) list: TemplateChild<gtk::ListBox>,

        reply_message: RefCell<Option<Message>>,

        manager: RefCell<Option<Manager>>,
        active_channel: RefCell<Option<Channel>>,
        last_signal_handler: RefCell<Option<SignalHandlerId>>,
    }

    #[gtk::template_callbacks]
    impl ChannelMessages {
        #[template_callback]
        fn send_message(&self, entry: gtk::Entry) {
            log::trace!("Got callback to send message");
            let text = entry.text();
            entry.set_text("");
            let obj = self.instance();
            if let Some(channel) = self.active_channel.borrow().as_ref() {
                log::trace!("Constructing message");
                let msg = Message::from_text_channel_sender(
                    text,
                    channel.clone(),
                    self.instance()
                        .property::<Manager>("manager")
                        .self_contact(),
                );
                if let Some(quote) = obj.property::<Option<Message>>("reply-message") {
                    log::trace!("Adding quote to message");
                    msg.set_quote(quote);
                }
                let main_context = MainContext::default();
                main_context.spawn_local(clone!(@strong msg, @strong channel => async move {
                    log::trace!("Sending message");
                    let _ = channel.send_message(msg).await;
                }));
            }
        }

        #[template_callback(function)]
        fn is_some(opt: Option<glib::Object>) -> bool {
            opt.is_some()
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
                    .unwrap_or("".to_string())
            );
            msg.set_property("expanded", !msg.property::<bool>("expanded"));
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
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ChannelMessages {
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
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "active-channel" => self.active_channel.borrow().as_ref().to_value(),
                "reply-message" => self.reply_message.borrow().as_ref().to_value(),
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
