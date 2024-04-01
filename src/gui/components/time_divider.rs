use crate::prelude::*;

glib::wrapper! {
    /// Divider shown between messages of different days.
    pub struct TimeDivider(ObjectSubclass<imp::TimeDivider>)
        @extends gtk::Box, gtk::Widget, @implements gtk::Accessible;
}

impl Default for TimeDivider {
    fn default() -> Self {
        Object::builder::<Self>().build()
    }
}

mod imp {
    use crate::prelude::*;

    use std::marker::PhantomData;

    use glib::subclass::InitializingObject;
    use gtk::subclass::box_::BoxImpl;

    use gtk::CompositeTemplate;

    use crate::backend::timeline::TimelineItem;

    #[derive(Debug, Default, CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::TimeDivider)]
    #[template(resource = "/ui/components/time_divider.ui")]
    pub struct TimeDivider {
        #[template_child]
        label_date: TemplateChild<gtk::Label>,

        #[property(set = Self::set_item)]
        item: PhantomData<TimelineItem>,
    }

    impl TimeDivider {
        fn set_item(&self, item: Option<TimelineItem>) {
            let formatted = item
                .and_then(|v| v.datetime())
                .and_then(|d| Utility::format_date_human(&d));
            self.label_date.set_text(&formatted.unwrap_or_default());
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TimeDivider {
        const NAME: &'static str = "TimeDivider";
        type Type = super::TimeDivider;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for TimeDivider {}

    impl WidgetImpl for TimeDivider {}
    impl BoxImpl for TimeDivider {}
}
