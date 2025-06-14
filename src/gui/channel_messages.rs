use crate::prelude::*;
use gio::SettingsBindFlags;

use crate::ApplicationError;

const MESSAGES_REQUEST_LOAD: usize = 10;

glib::wrapper! {
    /// [ChannelMessages] is the right pane displaying the list of messages and the entry-bar.
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

    /// The message list autoscrolls when the list is at the bottom.
    ///
    /// This furthermore loads the screen until is is completely filled.
    fn setup_autoscroll(&self) {
        let adj = self.imp().scrolled_window.vadjustment();
        adj.connect_value_changed(clone!(
            #[weak(rename_to = s)]
            self,
            move |adj| {
                s.set_sticky(adj.value() + adj.page_size() >= adj.upper());
            }
        ));
        adj.connect_upper_notify(clone!(
            #[weak(rename_to = s)]
            self,
            move |_adj| {
                if s.sticky() {
                    s.scroll_down();
                }
            }
        ));
        adj.connect_changed(clone!(
            #[weak(rename_to = s)]
            self,
            move |_adj| {
                s.load_if_screen_not_filled();
            }
        ));
        self.connect_notify_local(
            Some("active-channel"),
            clone!(
                #[weak(rename_to = s)]
                self,
                move |_, _| {
                    s.load_if_screen_not_filled();
                }
            ),
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
        crate::gspawn!(glib::clone!(
            #[strong(rename_to = s)]
            self,
            async move {
                // XXX: Need to sleep to prevent segfault: <https://gitlab.gnome.org/GNOME/gtk/-/issues/5763>
                glib::timeout_future(std::time::Duration::from_millis(100)).await;
                s.imp()
                    .scrolled_window
                    .emit_by_name::<bool>("scroll-child", &[&gtk::ScrollType::End, &false]);
            }
        ));
    }

    pub async fn clear_messages(&self) -> Result<(), ApplicationError> {
        if let Some(channel) = self.active_channel() {
            channel.clear_messages().await?;
        } else {
            log::warn!("Was asked to clear the messages with no currently active channel");
        }
        Ok(())
    }
}

pub mod imp {
    use std::marker::PhantomData;

    use crate::prelude::*;

    use glib::subclass::InitializingObject;
    use gtk::CompositeTemplate;
    use gtk::{FileDialog, PositionType, SignalListItemFactory};

    use crate::backend::timeline::Timeline;
    use crate::gui::attachment::backend_to_gui;
    use crate::gui::components::ItemRow;
    use crate::gui::components::time_divider::TimeDivider;
    use crate::{
        backend::{Channel, Manager, message::TextMessage},
        gui::{error_dialog::ErrorDialog, message_item::MessageItem, text_entry::TextEntry},
    };

    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[properties(wrapper_type = super::ChannelMessages)]
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
        #[template_child]
        button_send: TemplateChild<gtk::Button>,

        attachments: RefCell<Vec<crate::backend::Attachment>>,

        #[property(get, set = Self::set_active_channel)]
        active_channel: RefCell<Option<Channel>>,
        #[property(get, set, nullable)]
        reply_message: RefCell<Option<TextMessage>>,

        #[property(get, set, default = true)]
        sticky: Cell<bool>,
        #[property(get, set)]
        loading: Cell<bool>,
        #[property(get, set, default = true)]
        filling_screen: Cell<bool>,
        #[property(get = Self::has_attachments)]
        has_attachments: PhantomData<bool>,

        #[property(get, set = Self::set_manager, type = Manager)]
        manager: RefCell<Option<Manager>>,
    }

    #[gtk::template_callbacks]
    impl ChannelMessages {
        fn has_attachments(&self) -> bool {
            !self.attachments.borrow().is_empty()
        }

        fn set_manager(&self, man: Option<Manager>) {
            let initialized = man.is_some();
            self.manager.replace(man);
            if initialized {
                self.obj().setup_send_on_enter();
            }
        }

        fn set_active_channel(&self, chan: Option<Channel>) {
            if let Some(active_chan) = self.active_channel.borrow().as_ref() {
                active_chan.set_property("draft", self.text_entry.text());
            }

            let old = self.active_channel.replace(chan);
            if let Some(old) = old {
                old.trim_old();
            }
        }

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
                gspawn!(glib::clone!(
                    #[weak]
                    channel,
                    #[weak]
                    obj,
                    async move {
                        obj.set_loading(true);
                        channel.load_last(super::MESSAGES_REQUEST_LOAD).await;
                        obj.set_loading(false);
                    }
                ));
            } else {
                log::warn!(
                    "More messages were requested while not being focused on a channel. This should not happen."
                );
            }
        }

        #[template_callback]
        fn remove_reply(&self) {
            log::trace!("Unsetting reply message");
            self.obj().set_reply_message(None::<TextMessage>);
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

            // Due to (probably) a bug in GTK, a picture takes up pretty much the entire application when inserted into the box, sometimes even with tons of whitespace below.
            // Swapping `hexpand` off and on very quickly fixes this issue for some reason.
            // See https://gitlab.com/schmiddi-on-mobile/flare/-/issues/56 and https://gitlab.com/schmiddi-on-mobile/flare/-/issues/253.
            self.box_attachments.set_hexpand(true);
            gspawn!(clone!(
                #[strong(rename_to = box_attachments)]
                self.box_attachments,
                async move {
                    glib::timeout_future(std::time::Duration::from_millis(5)).await;
                    box_attachments.set_hexpand(false);
                }
            ));
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
                clone!(
                    #[strong]
                    obj,
                    move |file| {
                        if let Ok(file) = file {
                            log::trace!("User added an attachment");
                            obj.imp().paste_file(file);
                        } else {
                            log::trace!("User did not upload a attachment");
                        }
                    }
                ),
            );
        }

        #[template_callback]
        fn send_message(&self) {
            log::trace!("Got callback to send message");
            // Don't send if not allowed to. This can happen if the entry was activated and not the button.
            if !self.button_send.is_sensitive() {
                return;
            }
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
                    obj.set_reply_message(None::<TextMessage>);
                }

                let obj = self.obj();
                gspawn!(clone!(
                    #[strong]
                    msg,
                    #[strong]
                    channel,
                    #[strong]
                    attachments,
                    #[strong]
                    obj,
                    async move {
                        log::trace!("Adding attachments to message: {}", attachments.len());
                        for att in attachments {
                            if let Err(e) = msg.add_attachment(att).await {
                                let root = obj
                                    .root()
                                    .expect("`ChannelMessages` to have a root")
                                    .dynamic_cast::<crate::gui::Window>()
                                    .expect("Root of `ChannelMessages` to be a `Window`.");
                                let dialog = ErrorDialog::new(&e, &root);
                                dialog.present(Some(&root));
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
                            let dialog = ErrorDialog::new(&e, &root);
                            dialog.present(Some(&root));
                        }
                    }
                ));
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
        fn construct_list_view(&self) {
            let obj = self.obj();
            let factory = SignalListItemFactory::new();
            factory.connect_setup(clone!(
                #[weak]
                obj,
                move |_, object| {
                    let widget = ItemRow::default();
                    widget.connect_local(
                        "reply",
                        false,
                        clone!(
                            #[weak]
                            obj,
                            #[upgrade_or_default]
                            move |args| {
                                let msg = args[1].get::<Option<TextMessage>>().expect(
                                    "Type of signal `reply` of `ItemRow` to be `TextMessage`.",
                                );
                                obj.set_reply_message(msg);
                                obj.imp().text_entry.grab_focus();
                                None
                            }
                        ),
                    );
                    let list_item = object.downcast_ref::<gtk::ListItem>().unwrap();
                    list_item.set_activatable(false);
                    list_item.set_selectable(false);
                    list_item.set_child(Some(&widget));
                    list_item.bind_property("item", &widget, "item").build();
                }
            ));

            let header_factory = SignalListItemFactory::new();
            header_factory.connect_setup(move |_, object| {
                let widget = TimeDivider::default();
                let header_item = object.downcast_ref::<gtk::ListHeader>().unwrap();
                header_item.set_child(Some(&widget));
                header_item.bind_property("item", &widget, "item").build();
            });

            self.list_view.set_factory(Some(&factory));
            self.list_view.set_header_factory(Some(&header_factory));
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
            crate::backend::Contact::ensure_type();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ChannelMessages {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().connect_notify_local(
                Some("active-channel"),
                clone!(
                    #[weak(rename_to = s)]
                    self,
                    move |_, _| {
                        s.obj().set_reply_message(None::<TextMessage>);
                        if let Some(channel) = s.active_channel.borrow().as_ref() {
                            let draft = channel.property("draft");
                            s.text_entry.set_text(draft);
                        };
                    }
                ),
            );
            self.obj().setup_autoscroll();
            self.construct_list_view();
        }
    }

    impl WidgetImpl for ChannelMessages {}
    impl BoxImpl for ChannelMessages {}
}
