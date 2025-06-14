use crate::prelude::*;

glib::wrapper! {
    /// A divider between chats starting with different letters in the new chat dialog.
    pub struct LetterDivider(ObjectSubclass<imp::LetterDivider>)
        @extends gtk::Box, gtk::Widget, @implements gtk::Accessible;
}

impl Default for LetterDivider {
    fn default() -> Self {
        Object::builder::<Self>().build()
    }
}

mod imp {
    use std::marker::PhantomData;

    use crate::prelude::*;

    use glib::subclass::InitializingObject;
    use gtk::CompositeTemplate;
    use gtk::subclass::box_::BoxImpl;

    #[derive(Debug, Default, CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::LetterDivider)]
    #[template(resource = "/ui/components/letter_divider.ui")]
    pub struct LetterDivider {
        #[template_child]
        label_char: TemplateChild<gtk::Label>,

        #[property(set = Self::set_string)]
        string: PhantomData<String>,
    }

    impl LetterDivider {
        fn set_string(&self, string: Option<String>) {
            let formatted = string
                .and_then(|v| v.chars().next())
                .map(|c| c.to_uppercase().to_string());
            self.label_char.set_text(&formatted.unwrap_or_default());
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LetterDivider {
        const NAME: &'static str = "LetterDivider";
        type Type = super::LetterDivider;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for LetterDivider {}

    impl WidgetImpl for LetterDivider {}
    impl BoxImpl for LetterDivider {}
}
