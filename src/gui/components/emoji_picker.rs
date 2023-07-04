use glib::Object;
use gtk::glib;

gtk::glib::wrapper! {
    pub struct EmojiPicker(ObjectSubclass<imp::EmojiPicker>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl EmojiPicker {
    pub fn new() -> Self {
        log::trace!("Initializing `EmojiPicker`");
        Object::builder::<Self>().build()
    }
}

pub mod imp {

    use glib::subclass::Signal;
    use glib::{once_cell::sync::Lazy, subclass::InitializingObject};
    use gtk::glib;
    use gtk::{prelude::*, subclass::prelude::*, CompositeTemplate};

    use crate::gui::utility::Utility;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/components/emoji_picker.ui")]
    pub struct EmojiPicker {
        #[template_child]
        emoji_chooser: TemplateChild<gtk::EmojiChooser>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EmojiPicker {
        const NAME: &'static str = "FlEmojiPicker";
        type Type = super::EmojiPicker;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl EmojiPicker {
        #[template_callback]
        pub(super) fn handle_react_open(&self) {
            crate::trace!("Opening emoji dropdown",);
            let obj = self.obj();

            self.emoji_chooser.popup();
        }

        #[template_callback]
        pub(super) fn reacted(&self, emoji: String) {
            let obj = self.obj();
            obj.emit_by_name::<()>("reacted", &[&emoji]);
        }

        #[template_callback]
        pub(super) fn button_react(&self, button: gtk::Button) {
            let emoji = button.label();
            if let Some(emoji) = emoji {
                self.reacted(emoji.to_string());
            }
        }
    }

    impl ObjectImpl for EmojiPicker {
        fn constructed(&self) {
            self.parent_constructed();
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("reacted")
                    .param_types([String::static_type()])
                    .build()]
            });
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for EmojiPicker {}
    impl BoxImpl for EmojiPicker {}
}
