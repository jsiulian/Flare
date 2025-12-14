use crate::prelude::*;

use crate::gui::attachment::Attachment;

gtk::glib::wrapper! {
    /// Attachment displaying an image.
    pub struct AttachmentImage(ObjectSubclass<imp::AttachmentImage>)
        @extends gtk::Widget, Attachment,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl AttachmentImage {
    pub fn new(attachment: &crate::backend::Attachment) -> Self {
        log::trace!("Initializing `Attachment`");
        Object::builder::<Self>()
            .property("attachment", attachment)
            .build()
    }
}

pub mod imp {
    use crate::prelude::*;

    use glib::subclass::InitializingObject;
    use gtk::{CompositeTemplate, Picture};

    use crate::gui::{attachment::Attachment, attachment::AttachmentImpl, utility::Utility};

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/components/attachment_image.ui")]
    pub struct AttachmentImage {
        #[template_child]
        picture: TemplateChild<Picture>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AttachmentImage {
        const NAME: &'static str = "FlAttachmentImage";
        type Type = super::AttachmentImage;
        type ParentType = Attachment;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AttachmentImage {
        fn constructed(&self) {
            self.parent_constructed();
        }

        fn dispose(&self) {
            self.picture.unparent()
        }
    }

    impl WidgetImpl for AttachmentImage {}
    impl AttachmentImpl for AttachmentImage {}
}
