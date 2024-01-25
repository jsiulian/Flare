use gdk::glib::{Priority, Propagation};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::subclass::widget::WidgetImpl;
use gtk::{gdk, gio, glib, CompositeTemplate, TemplateChild, TextView};

use crate::gspawn;
use crate::gui::utility::Utility;
use glib::{
    clone,
    once_cell::sync::Lazy,
    subclass::{InitializingObject, Signal},
    ParamSpec, ParamSpecBoolean, ParamSpecObject, Value,
};
use std::cell::{Cell, RefCell};

glib::wrapper! {
    pub struct TextEntry(ObjectSubclass<imp::TextEntry>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl TextEntry {
    pub fn text(&self) -> String {
        let buffer: sourceview5::Buffer = self.property("buffer");
        let (start_iter, end_iter) = buffer.bounds();
        buffer.text(&start_iter, &end_iter, true).to_string()
    }

    pub fn clear(&self) {
        let buffer: sourceview5::Buffer = self.property("buffer");
        buffer.set_text("");
    }

    pub fn insert_emoji(&self) {
        self.imp().insert_emoji();
    }

    pub fn send_on_enter(&self) -> bool {
        self.property("send-on-enter")
    }
}

pub mod imp {

    use super::*;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/text_entry.ui")]
    pub struct TextEntry {
        #[template_child]
        pub(super) view: TemplateChild<TextView>,

        pub(super) buffer: RefCell<Option<sourceview5::Buffer>>,

        send_on_enter: Cell<bool>,
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

    impl ObjectImpl for TextEntry {
        fn constructed(&self) {
            let obj = self.obj();
            obj.set_property("buffer", sourceview5::Buffer::new(None));
            let key_events = gtk::EventControllerKey::new();
            key_events
                .connect_key_pressed(clone!(@weak obj => @default-return Propagation::Proceed, move |_, key, _, modifier| {
                let enter_pressed = key == gdk::Key::Return || key == gdk::Key::KP_Enter;
                let should_send = enter_pressed && if obj.send_on_enter() {
                    // shift+enter / ctrl+enter yields newline, enter sends
                    !modifier.contains(gdk::ModifierType::CONTROL_MASK) && !modifier.contains(gdk::ModifierType::SHIFT_MASK)
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
            }));
            self.view.add_controller(key_events);

            self.view
                .connect_paste_clipboard(clone!(@weak obj => move |entry| {
                    let clipboard = obj.clipboard();
                    let formats = clipboard.formats();

                    // We only handle files and supported images.
                    gspawn!(clone!(@weak entry => async move {
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
                                    Err(error) => log::warn!("Could not get file from value: {error:?}"),
                                },
                                Err(error) => log::warn!("Could not get file from the clipboard: {error:?}"),
                            }
                        } else if formats.contains_type(gdk::Texture::static_type()) {
                            entry.stop_signal_emission_by_name("paste-clipboard");
                            match clipboard
                                .read_value_future(gdk::Texture::static_type(), Priority::DEFAULT)
                                .await
                            {
                                Ok(value) => match value.get::<gdk::Texture>() {
                                    Ok(texture) => {
                                        obj.emit_by_name::<()>("paste-texture", &[&texture]);
                                    }
                                    Err(error) => log::warn!("Could not get file from value: {error:?}"),
                                },
                                Err(error) => log::warn!("Could not get file from the clipboard: {error:?}"),
                            }
                        }
                    }));
                }));

            let buffer: sourceview5::Buffer = obj.property("buffer");
            obj.imp().view.set_buffer(Some(&buffer));
            buffer.connect_text_notify(clone!(@weak obj => move |_| {
                obj.notify("is-empty");
            }));

            #[cfg(feature = "libspelling")]
            {
                let checker = libspelling::Checker::default();
                let adapter = libspelling::TextBufferAdapter::new(&buffer, &checker);
                let extra_menu = adapter.menu_model();

                self.view.set_extra_menu(Some(&extra_menu));
                self.view.insert_action_group("spelling", Some(&adapter));

                adapter.set_enabled(true);
            }
        }
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecBoolean::builder("is-empty").read_only().build(),
                    ParamSpecBoolean::builder("send-on-enter").build(),
                    ParamSpecObject::builder::<sourceview5::Buffer>("buffer").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "is-empty" => {
                    let (start, end) = self.buffer.borrow().as_ref()
                    .expect("Property `Buffer` of `TextEntry` has to be of type `sourceview5::Buffer`")
                    .bounds();
                    (start == end).to_value()
                }
                "send-on-enter" => self.send_on_enter.get().to_value(),
                "buffer" => self.buffer.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "send-on-enter" => {
                    let b = value
                        .get::<bool>()
                        .expect("Property `send-on-enter` of `TextEntry` has to be of type `bool`");
                    self.send_on_enter.replace(b);
                }
                "buffer" => {
                    let obj = value.get::<Option<sourceview5::Buffer>>().expect(
                        "Property `Buffer` of `TextEntry` has to be of type `sourceview5::Buffer`",
                    );

                    self.buffer.replace(obj);
                }
                _ => unimplemented!(),
            }
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
