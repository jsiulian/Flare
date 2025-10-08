use crate::prelude::*;

use sourceview5::Buffer;

glib::wrapper! {
    /// Widget for entering text messages.
    ///
    /// This adds a few nice-to-have features to the TextView, like sending on enter (configurable), spell checking (if installed) or pasting files.
    pub struct TextEntry(ObjectSubclass<imp::TextEntry>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl TextEntry {
    pub fn buffer(&self) -> Buffer {
        self.imp().view.buffer().downcast().unwrap()
    }

    pub fn text(&self) -> String {
        let buffer: Buffer = self.buffer();
        let (start_iter, end_iter) = buffer.bounds();
        buffer.text(&start_iter, &end_iter, true).to_string()
    }

    pub fn set_text(&self, text: String) {
        let buffer = self.buffer();
        buffer.set_text(text.as_str());
    }

    pub fn clear(&self) {
        let buffer: Buffer = self.buffer();
        buffer.set_text("");
    }

    pub fn insert_emoji(&self) {
        self.imp().insert_emoji();
    }
}

pub mod imp {
    use crate::prelude::*;

    use glib::subclass::{InitializingObject, Signal};
    use glib::{Priority, Propagation};
    use gtk::subclass::widget::WidgetImpl;
    use gtk::{CompositeTemplate, InputHints, TemplateChild, TextView};

    use std::cell::Cell;

    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[template(resource = "/ui/text_entry.ui")]
    #[properties(wrapper_type = super::TextEntry)]
    pub struct TextEntry {
        #[template_child]
        pub(super) view: TemplateChild<TextView>,

        #[property(get, set)]
        send_on_enter: Cell<bool>,
        #[property(get, set)]
        is_empty: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TextEntry {
        const NAME: &'static str = "FlTextEntry";
        type Type = super::TextEntry;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
            klass.set_css_name("message-entry");
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }
    #[gtk::template_callbacks]
    impl TextEntry {
        #[template_callback]
        pub(super) fn insert_emoji(&self) {
            self.view.emit_insert_emoji();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for TextEntry {
        fn constructed(&self) {
            let obj = self.obj();

            // Send on enter
            let key_events = gtk::EventControllerKey::new();
            key_events.connect_key_pressed(clone!(
                #[weak]
                obj,
                #[upgrade_or]
                Propagation::Proceed,
                move |_, key, _, modifier| {
                    let enter_pressed = key == gdk::Key::Return || key == gdk::Key::KP_Enter;
                    let should_send = enter_pressed
                        && if obj.send_on_enter() {
                            // shift+enter / ctrl+enter yields newline, enter sends
                            !modifier.contains(gdk::ModifierType::CONTROL_MASK)
                                && !modifier.contains(gdk::ModifierType::SHIFT_MASK)
                        } else {
                            // enter / shift+enter yields newline, ctrl+enter sends
                            modifier.contains(gdk::ModifierType::CONTROL_MASK)
                        };
                    if should_send {
                        obj.emit_by_name::<()>("activate", &[]);
                        Propagation::Stop
                    } else {
                        Propagation::Proceed
                    }
                }
            ));
            self.view.add_controller(key_events);

            // Paste files
            self.view.connect_paste_clipboard(clone!(
                #[weak]
                obj,
                move |entry| {
                    let clipboard = obj.clipboard();
                    let formats = clipboard.formats();

                    // We only handle files and supported images.
                    gspawn!(clone!(
                        #[weak]
                        entry,
                        async move {
                            if formats.contains_type(gio::File::static_type()) {
                                entry.stop_signal_emission_by_name("paste-clipboard");
                                match clipboard
                                    .read_value_future(gio::File::static_type(), Priority::DEFAULT)
                                    .await
                                {
                                    Ok(value) => match value.get::<gio::File>() {
                                        Ok(file) => {
                                            obj.emit_by_name::<()>("paste-file", &[&file]);
                                        }
                                        Err(error) => {
                                            log::warn!("Could not get file from value: {error:?}")
                                        }
                                    },
                                    Err(error) => log::warn!(
                                        "Could not get file from the clipboard: {error:?}"
                                    ),
                                }
                            } else if formats.contains_type(gdk::Texture::static_type()) {
                                entry.stop_signal_emission_by_name("paste-clipboard");
                                match clipboard
                                    .read_value_future(
                                        gdk::Texture::static_type(),
                                        Priority::DEFAULT,
                                    )
                                    .await
                                {
                                    Ok(value) => match value.get::<gdk::Texture>() {
                                        Ok(texture) => {
                                            obj.emit_by_name::<()>("paste-texture", &[&texture]);
                                        }
                                        Err(error) => {
                                            log::warn!("Could not get file from value: {error:?}")
                                        }
                                    },
                                    Err(error) => log::warn!(
                                        "Could not get file from the clipboard: {error:?}"
                                    ),
                                }
                            }
                        }
                    ));
                }
            ));

            // Updating property if empty.
            let buffer = sourceview5::Buffer::new(None);
            let view = &obj.imp().view;
            view.set_buffer(Some(&buffer));
            buffer.connect_text_notify(clone!(
                #[weak]
                obj,
                move |_| {
                    let (start, end) = obj.buffer().bounds();
                    obj.set_is_empty(start == end)
                }
            ));
            view.connect_preedit_changed(clone!(
                #[weak]
                obj,
                move |_, preedit| {
                    let (start, end) = obj.buffer().bounds();
                    obj.set_is_empty(preedit.is_empty() && start == end)
                }
            ));

            // Spell checking.
            #[cfg(feature = "libspelling")]
            {
                let checker = libspelling::Checker::default();
                let adapter = libspelling::TextBufferAdapter::new(&buffer, &checker);
                let extra_menu = adapter.menu_model();

                self.view.set_extra_menu(Some(&extra_menu));
                self.view.insert_action_group("spelling", Some(&adapter));

                adapter.set_enabled(true);
            }

            // Set input hints
            // For some reason, setting it via blueprint only sets the first hint, but not both at once.
            self.view
                .set_input_hints(InputHints::WORD_COMPLETION | InputHints::SPELLCHECK);

            // For some reason, setting via `default = true` in property does not work.
            self.obj().set_is_empty(true);
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| -> Vec<Signal> {
                vec![
                    Signal::builder("activate").build(),
                    Signal::builder("paste-file")
                        .param_types([gio::File::static_type()])
                        .build(),
                    Signal::builder("paste-texture")
                        .param_types([gdk::Texture::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for TextEntry {
        fn grab_focus(&self) -> bool {
            log::trace!("TextEntry grabbed focus");
            self.view.grab_focus()
        }
    }
    impl BoxImpl for TextEntry {}
}
