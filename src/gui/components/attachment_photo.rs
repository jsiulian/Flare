use glib::Object;
use gtk::glib;

use crate::gui::attachment::Attachment;

gtk::glib::wrapper! {
    pub struct AttachmentPhoto(ObjectSubclass<imp::AttachmentPhoto>)
        @extends gtk::Widget, Attachment;
}

impl AttachmentPhoto {
    pub fn new(attachment: &crate::backend::Attachment) -> Self {
        log::trace!("Initializing `Attachment`");
        Object::builder::<Self>()
            .property("attachment", attachment)
            .build()
    }
}

pub mod imp {
    use glib::subclass::InitializingObject;
    use gtk::prelude::WidgetExt;
    use gtk::{glib, Picture};
    use gtk::{subclass::prelude::*, CompositeTemplate};

    use crate::gui::{attachment::Attachment, attachment::AttachmentImpl, utility::Utility};

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/components/attachment_photo.ui")]
    pub struct AttachmentPhoto {
        #[template_child]
        picture: TemplateChild<Picture>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AttachmentPhoto {
        const NAME: &'static str = "FlAttachmentPhoto";
        type Type = super::AttachmentPhoto;
        type ParentType = Attachment;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AttachmentPhoto {
        fn constructed(&self) {
            self.parent_constructed();
        }

        fn dispose(&self) {
            self.picture.unparent()
        }
    }

    impl WidgetImpl for AttachmentPhoto {}
    impl AttachmentImpl for AttachmentPhoto {}
}
