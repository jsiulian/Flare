use crate::gui::attachment::Attachment;
use glib::Object;
use gtk::glib;

gtk::glib::wrapper! {
    pub struct AttachmentFile(ObjectSubclass<imp::AttachmentFile>)
        @extends gtk::Widget, Attachment;
}

impl AttachmentFile {
    pub fn new(attachment: &crate::backend::Attachment) -> Self {
        log::trace!("Initializing `Attachment`");
        Object::builder::<Self>()
            .property("attachment", attachment)
            .build()
    }
}

pub mod imp {

    use crate::gui::{attachment::Attachment, attachment::AttachmentImpl, utility::Utility};
    use glib::subclass::InitializingObject;
    use gtk::glib;
    use gtk::{prelude::*, subclass::prelude::*, CompositeTemplate};

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/components/attachment_file.ui")]
    pub struct AttachmentFile {
        #[template_child]
        box_file: TemplateChild<gtk::Grid>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AttachmentFile {
        const NAME: &'static str = "FlAttachmentFile";
        type Type = super::AttachmentFile;
        type ParentType = Attachment;

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
    impl AttachmentFile {
        #[template_callback]
        pub fn download(&self) {
            let obj = self.obj();
            let att: Attachment = obj.clone().upcast();
            let att_backend: crate::backend::Attachment = att.property("attachment");
            if !att_backend.property::<bool>("loaded") {
                att.imp().load();
            }
            att.imp().download();
        }
    }

    impl ObjectImpl for AttachmentFile {
        fn constructed(&self) {
            self.parent_constructed();
        }

        fn dispose(&self) {
            self.box_file.unparent()
        }
    }

    impl WidgetImpl for AttachmentFile {}
    impl AttachmentImpl for AttachmentFile {}
}
