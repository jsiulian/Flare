use crate::prelude::*;

use glib::subclass::types::ObjectSubclassIsExt;

use crate::backend::message::{CallMessage, CallMessageType};
use crate::backend::Contact;

gtk::glib::wrapper! {
    /// A widget to display call events.
    pub struct CallMessageItem(ObjectSubclass<imp::CallMessageItem>)
        @extends gtk::Box,gtk::Widget;
}

impl CallMessageItem {
    pub fn new(message: &CallMessage) -> Self {
        log::trace!("Initializing `CallMessageItem`");
        let obj = Object::builder::<Self>()
            .property("message", message)
            .build();

        let sender: Contact = message.property("sender");

        let icon_name = match (message.call_type(), sender.is_self()) {
            (CallMessageType::Offer, false) => "call-incoming-symbolic",
            (CallMessageType::Offer, true) => "call-outcoming-symbolic",
            (CallMessageType::Answer, _) => "call-start-symbolic",
            (CallMessageType::Hangup, _) => "call-stop-symbolic",
            (CallMessageType::Busy, _) => "call-missed-symbolic",
        };

        obj.imp().icon.set_icon_name(Some(icon_name));
        obj
    }
}

pub mod imp {
    use crate::prelude::*;

    use glib::subclass::InitializingObject;
    use gtk::CompositeTemplate;

    use crate::backend::{message::CallMessage, Manager};

    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[properties(wrapper_type = super::CallMessageItem)]
    #[template(resource = "/ui/call_message_item.ui")]
    pub struct CallMessageItem {
        #[template_child]
        pub icon: TemplateChild<gtk::Image>,

        #[property(get, set)]
        message: RefCell<Option<CallMessage>>,

        #[property(get, set, construct_only, type = Manager)]
        manager: RefCell<Option<Manager>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CallMessageItem {
        const NAME: &'static str = "FlCallMessageItem";
        type Type = super::CallMessageItem;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for CallMessageItem {}

    impl WidgetImpl for CallMessageItem {}
    impl BoxImpl for CallMessageItem {}
}
