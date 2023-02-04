use gtk::glib;
use glib::subclass::prelude::{ObjectSubclassIsExt};
use gtk::traits::TextBufferExt;

gtk::glib::wrapper! {
    pub struct TextEntry(ObjectSubclass<imp::TextEntry>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl TextEntry {
    pub fn text(&self) -> String {
        let obj = self.imp();
        let buffer = &obj.buffer;
        let (start_iter, end_iter) = buffer.bounds();
        buffer.text(&start_iter, &end_iter, true).to_string()
    }

    pub fn clear(&self) {
        let obj = self.imp();
        let buffer = &obj.buffer;
        buffer.set_text("");
    }
}

pub mod imp {
    use crate::gspawn;
    use gtk::{gdk, gio, glib};
    use glib::{
        prelude::{ObjectExt, ToValue},
        subclass::prelude::{ObjectImpl, ObjectSubclass, ObjectSubclassExt},
    };
    use glib::{
        clone,
        once_cell::sync::Lazy,
        subclass::{InitializingObject, Signal},
        ParamFlags, ParamSpec, ParamSpecBoolean, Value,
    };
    use gtk::{
        prelude::{InitializingWidgetExt, StaticType},
        subclass::{
            prelude::BoxImpl,
            widget::{CompositeTemplate, WidgetClassSubclassExt, WidgetImpl},
        },
        traits::{TextBufferExt, TextViewExt, WidgetExt},
        CompositeTemplate, Inhibit, TemplateChild, TextBuffer, TextView,
    };

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/text_entry.ui")]
    pub struct TextEntry {
        #[template_child]
        pub(super) view: TemplateChild<TextView>,
        #[template_child]
        pub(super) buffer: TemplateChild<TextBuffer>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TextEntry {
        const NAME: &'static str = "FlTextEntry";
        type Type = super::TextEntry;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for TextEntry {
        fn constructed(&self) {
            let obj = self.obj();
            let key_events = gtk::EventControllerKey::new();
            self.view.add_controller(&key_events);
            key_events
                .connect_key_pressed(clone!(@weak obj => @default-return Inhibit(false), move |_, key, _, modifier| {
                if !modifier.contains(gdk::ModifierType::SHIFT_MASK) && (key == gdk::Key::Return || key == gdk::Key::KP_Enter) {
                    obj.emit_by_name::<()>("activate", &[]);
                    Inhibit(true)
                } else {
                    Inhibit(false)
                }
            }));

            self.view
                .connect_paste_clipboard(clone!(@weak obj => move |entry| {
                    let clipboard = obj.clipboard();
                    let formats = clipboard.formats();

                    // We only handle files and supported images.
                    gspawn!(clone!(@weak entry => async move {
                        if formats.contains_type(gio::File::static_type()) {
                            entry.stop_signal_emission_by_name("paste-clipboard");
                            match clipboard
                                .read_value_future(gio::File::static_type(), glib::PRIORITY_DEFAULT)
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
                                .read_value_future(gdk::Texture::static_type(), glib::PRIORITY_DEFAULT)
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

            self.buffer
                .connect_text_notify(clone!(@weak obj => move |_| {
                    obj.notify("is-empty");
                }));
        }
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![ParamSpecBoolean::new(
                    "is-empty",
                    "is-empty",
                    "is-empty",
                    false,
                    ParamFlags::READABLE,
                )]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "is-empty" => {
                    let (start, end) = self.buffer.bounds();
                    (start == end).to_value()
                }
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, _value: &Value, _pspec: &ParamSpec) {
            unimplemented!()
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| -> Vec<Signal> {
                vec![
                    Signal::builder("activate")
                        .build(),
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
