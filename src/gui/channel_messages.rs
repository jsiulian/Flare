use gdk::gio::SettingsBindFlags;
use gdk::glib::clone;
use gdk::prelude::SettingsExtManual;
use gdk::subclass::prelude::ObjectSubclassIsExt;
use glib::ObjectExt;
use gtk::traits::{AdjustmentExt, WidgetExt};
use gtk::{gdk, glib};

use crate::backend::{message::TextMessage, Channel, Manager};

const MESSAGES_REQUEST_LOAD: usize = 10;

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

    pub fn sticky(&self) -> bool {
        self.property("sticky")
    }

    pub fn set_sticky(&self, val: bool) {
        self.set_property("sticky", val)
    }

    pub fn loading(&self) -> bool {
        self.property("loading")
    }

    pub fn set_loading(&self, val: bool) {
        self.set_property("loading", val)
    }

    pub fn filling_screen(&self) -> bool {
        self.property("filling-screen")
    }

    pub fn set_filling_screen(&self, val: bool) {
        self.set_property("filling-screen", val)
    }

    /// If the screen is not yet fully filled with messages, fill it.
    /// If the screen was just filled, scroll down.
    fn load_if_screen_not_filled(&self) {
        let adj = self.imp().scrolled_window.vadjustment();
        if self.active_channel().is_some() && adj.upper() <= adj.page_size() {
            // The screen is not yet filled.
            self.set_filling_screen(true);
            self.imp().handle_more();
        } else if self.filling_screen() {
            // Just filled the screen.
            self.set_filling_screen(false);
            self.scroll_down();
        }
    }

    fn setup_autoscroll(&self) {
        let adj = self.imp().scrolled_window.vadjustment();
        adj.connect_value_changed(clone!(@weak self as s => move |adj| {
            s.set_sticky(adj.value() + adj.page_size() >= adj.upper());
        }));
        adj.connect_upper_notify(clone!(@weak self as s => move |_adj| {
            if s.sticky() {
                s.scroll_down();
            }
        }));
        adj.connect_changed(clone!(@weak self as s => move |_adj| {
            s.load_if_screen_not_filled();
        }));
        self.connect_notify_local(
            Some("active-channel"),
            clone!(@weak self as s => move |_, _| {
                s.load_if_screen_not_filled();
            }),
        );
    }

    fn setup_send_on_enter(&self) {
        self.manager()
            .settings()
            .bind(
                "send-on-enter",
                &self.imp().text_entry.get(),
                "send-on-enter",
            )
            .flags(SettingsBindFlags::GET)
            .build();
    }

    fn scroll_down(&self) {
        crate::gspawn!(glib::clone!(@strong self as s => async move  {
            // XXX: Need to sleep to prevent segfault: <https://gitlab.gnome.org/GNOME/gtk/-/issues/5763>
            glib::timeout_future(std::time::Duration::from_millis(100)).await;
            s.imp()
                .scrolled_window
                .emit_by_name::<bool>("scroll-child", &[&gtk::ScrollType::End, &false]);
        }));
    }
}

pub mod imp {
    use std::cell::{Cell, RefCell};

    use glib::{
        clone, once_cell::sync::Lazy, subclass::InitializingObject, ParamSpec, ParamSpecBoolean,
        ParamSpecObject, Value,
    };
    use gtk::{gio, glib, FileDialog, PositionType, SignalListItemFactory};
    use gtk::{prelude::*, subclass::prelude::*, CompositeTemplate};

    use crate::backend::timeline::Timeline;
    use crate::gui::attachment::backend_to_gui;
    use crate::gui::components::ItemRow;
    use crate::{
        backend::{message::TextMessage, Channel, Manager},
        gspawn,
        gui::{
            error_dialog::ErrorDialog, message_item::MessageItem, text_entry::TextEntry,
            utility::Utility,
        },
    };

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/channel_messages.ui")]
    pub struct ChannelMessages {
        #[template_child]
        pub(super) scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        box_attachments: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) text_entry: TemplateChild<TextEntry>,
        #[template_child]
        pub(super) list_view: TemplateChild<gtk::ListView>,

        attachments: RefCell<Vec<crate::backend::Attachment>>,
        reply_message: RefCell<Option<TextMessage>>,

        manager: RefCell<Option<Manager>>,
        active_channel: RefCell<Option<Channel>>,

        sticky: Cell<bool>,
        loading: Cell<bool>,
        filling_screen: Cell<bool>,
    }

    #[gtk::template_callbacks]
    impl ChannelMessages {
        #[template_callback(function)]
        fn no_selection(timeline: Option<Timeline>) -> gtk::SelectionModel {
            gtk::NoSelection::new(timeline).into()
        }

        #[template_callback]
        fn scroll_down(&self) {
            self.obj().scroll_down()
        }

        #[template_callback]
        fn handle_edge_reached(&self, position: PositionType) {
            if position == PositionType::Top {
                self.handle_more()
            }
        }

        #[template_callback]
        pub(super) fn handle_more(&self) {
            log::trace!("More messages were requested in the UI");
            let channel = self.active_channel.borrow();
            if let Some(channel) = channel.as_ref() {
                let obj = self.obj();
                gspawn!(glib::clone!(@weak channel, @weak obj => async move {
                    obj.set_loading(true);
                    channel.load_last(super::MESSAGES_REQUEST_LOAD).await;
                    obj.set_loading(false);
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
            // File attachments can only be sent alone and do not allow any other attachments to be added.
            if attachment.is_file() || self.attachments.borrow().iter().any(|a| a.is_file()) {
                self.remove_attachments();
            }
            let att_widget = backend_to_gui(&attachment);
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
            let chooser = FileDialog::builder().build();
            let obj = self.obj();
            chooser.open(
                Some(
                    &self
                        .obj()
                        .root()
                        .expect("`ChannelMessages` to have a root")
                        .dynamic_cast::<crate::gui::Window>()
                        .expect("Root of `ChannelMessages` to be a `Window`."),
                ),
                None::<&gio::Cancellable>,
                clone!(@strong chooser, @strong obj => move |file| {
                    if let Ok(file) = file{
                        log::trace!("User added an attachment");
                        obj.imp().paste_file(file);
                    } else {
                        log::trace!("User did not upload a attachment");
                    }
                }),
            );
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
                                dialog.present();
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
                            dialog.present();
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

        #[template_callback]
        fn insert_emoji(&self) {
            self.text_entry.insert_emoji();
        }
    }

    impl ChannelMessages {
        fn construct_list_view(&self) {
            let obj = self.obj();
            let factory = SignalListItemFactory::new();
            factory.connect_setup(clone!(@weak obj => move |_, object| {
                let widget = ItemRow::default();
                widget.connect_local("reply", false, clone!(@weak obj => @default-return None, move |args| {
                    let msg = args[1]
                        .get::<Option<TextMessage>>()
                        .expect("Type of signal `reply` of `ItemRow` to be `TextMessage`.");
                    obj.set_reply_message(&msg);
                    None
                }));
                let list_item = object.downcast_ref::<gtk::ListItem>().unwrap();
                list_item.set_activatable(false);
                list_item.set_selectable(false);
                list_item.set_child(Some(&widget));
                list_item.bind_property("item", &widget, "item").build();
            }));

            self.list_view.set_factory(Some(&factory));
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
            self.obj().setup_autoscroll();
            self.construct_list_view();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<Manager>("manager").build(),
                    ParamSpecObject::builder::<Channel>("active-channel").build(),
                    ParamSpecObject::builder::<TextMessage>("reply-message").build(),
                    ParamSpecBoolean::builder("has-attachments").build(),
                    ParamSpecBoolean::builder("sticky")
                        .default_value(true)
                        .build(),
                    ParamSpecBoolean::builder("loading").build(),
                    ParamSpecBoolean::builder("filling-screen")
                        .default_value(true)
                        .build(),
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
                "sticky" => self.sticky.get().to_value(),
                "loading" => self.loading.get().to_value(),
                "filling-screen" => self.filling_screen.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let man = value.get::<Option<Manager>>().expect(
                        "Property `manager` of `ChannelMessages` has to be of type `Manager`",
                    );
                    let initialized = man.is_some();
                    self.manager.replace(man);
                    if initialized {
                        self.obj().setup_send_on_enter();
                    }
                }
                "active-channel" => {
                    let chan = value.get::<Option<Channel>>().expect(
                        "Property `active-channel` of `ChannelMessages` has to be of type `Channel`",
                    );
                    let old = self.active_channel.replace(chan);
                    if let Some(old) = old {
                        old.trim_old();
                    }
                }
                "reply-message" => {
                    let msg = value.get::<Option<TextMessage>>().expect(
                        "Property `reply-message` of `ChannelMessages` has to be of type `TextMessage`",
                    );
                    self.reply_message.replace(msg);
                }
                "sticky" => {
                    let s = value
                        .get::<bool>()
                        .expect("Property `sticky` of `ChannelMessages` has to be of type `bool`");
                    self.sticky.replace(s);
                }
                "loading" => {
                    let l = value
                        .get::<bool>()
                        .expect("Property `loading` of `ChannelMessages` has to be of type `bool`");
                    self.loading.replace(l);
                }
                "filling-screen" => {
                    let f = value.get::<bool>().expect(
                        "Property `filling-screen` of `ChannelMessages` has to be of type `bool`",
                    );
                    self.filling_screen.replace(f);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for ChannelMessages {}
    impl BoxImpl for ChannelMessages {}
}
