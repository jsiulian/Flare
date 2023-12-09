use glib::Object;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;

mod imp {
    use super::*;
    use std::cell::RefCell;
    #[derive(Default, CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::MessageIndicators)]
    #[template(resource = "/ui/components/indicators.ui")]
    pub struct MessageIndicators {
        #[property(get, set)]
        pub(super) timestamp: RefCell<String>,
        //TODO: Implement sending state
        //#[template_child]
        //pub(super) sending_state_icon: TemplateChild<gtk::Image>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageIndicators {
        const NAME: &'static str = "FlMessageIndicators";
        type Type = super::MessageIndicators;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("messageindicators");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }
    #[glib::derived_properties]
    impl ObjectImpl for MessageIndicators {
        fn constructed(&self) {
            self.parent_constructed();
        }

        fn dispose(&self) {
            let mut child = self.obj().first_child();
            while let Some(child_) = child {
                child = child_.next_sibling();
                child_.unparent();
            }
        }
    }

    impl WidgetImpl for MessageIndicators {}
}

glib::wrapper! {
    pub struct MessageIndicators(ObjectSubclass<imp::MessageIndicators>)
        @extends gtk::Widget;
}

impl MessageIndicators {
    pub fn new(timestamp: String) -> Self {
        log::trace!("Initializing `MessageIndicators`");
        Object::builder::<Self>()
            .property("timestamp", timestamp)
            .build()
    }
}
