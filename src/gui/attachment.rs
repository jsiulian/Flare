use gdk_pixbuf::glib::Object;

gtk::glib::wrapper! {
    pub struct Attachment(ObjectSubclass<imp::Attachment>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl Attachment {
    pub fn new(attachment: &crate::backend::Attachment) -> Self {
        log::trace!("Initializing `Attachment`");
        Object::new(&[("attachment", attachment)]).expect("Failed to create `Attachment`")
    }
}

pub mod imp {
    use std::cell::RefCell;

    use gdk_pixbuf::glib::clone;
    use gdk_pixbuf::glib::once_cell::sync::Lazy;
    use gdk_pixbuf::glib::MainContext;
    use gdk_pixbuf::glib::ParamFlags;
    use gdk_pixbuf::glib::ParamSpec;
    use gdk_pixbuf::glib::ParamSpecObject;
    use gdk_pixbuf::glib::Value;
    use glib::subclass::InitializingObject;
    use gtk::builders::FileChooserNativeBuilder;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::CompositeTemplate;
    use gtk::FileChooserAction;
    use gtk::ResponseType;

    use crate::backend::Manager;
    use crate::gui::error_dialog::ErrorDialog;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/attachment.ui")]
    pub struct Attachment {
        attachment: RefCell<Option<crate::backend::Attachment>>,

        manager: RefCell<Option<Manager>>,
    }

    #[gtk::template_callbacks]
    impl Attachment {
        #[template_callback]
        fn download(&self, _: gtk::Button) {
            log::trace!("User requested to dowload attachment");
            if let Some(attachment) = self.attachment.borrow().as_ref() {
                let chooser = FileChooserNativeBuilder::new()
                    .transient_for(
                        &self
                            .instance()
                            .root()
                            .expect("`Attachment` to have a root")
                            .dynamic_cast::<crate::gui::Window>()
                            .expect("Root of `Attachment` to be a `Window`."),
                    )
                    .action(FileChooserAction::Save)
                    .build();
                if let Some(name) = attachment.name() {
                    crate::trace!("Setting filename to {:?}", &name);
                    chooser.set_current_name(&name);
                }

                let obj = self.instance();
                chooser.connect_response(
                    clone!(@strong chooser, @strong attachment, @strong obj => move |_, action| {
                        if action == ResponseType::Accept {
                            log::trace!("User downloads attachment");
                            let file = chooser.file();
                            if let Some(file) = file {
                                let main_context = MainContext::default();
                                main_context.spawn_local(clone!(@strong attachment, @strong obj => async move {
                                    if let Err(e) = attachment.save_to_file(&file).await {
                                        let root = obj
                                            .root()
                                            .expect("`Attachment` to have a root")
                                            .dynamic_cast::<crate::gui::Window>()
                                            .expect("Root of `Attachment` to be a `Window`.");
                                        let dialog = ErrorDialog::new(e.into(), &root);
                                        dialog.show();
                                    }
                                }));
                            } else {
                                log::trace!("Got no file to save the attachment to");
                            }
                        } else {
                            log::trace!("User did not save a attachment");
                        }
                    }),
                );
                log::trace!("Showing download popup");
                chooser.show();
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Attachment {
        const NAME: &'static str = "FlAttachment";
        type Type = super::Attachment;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Attachment {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::new(
                        "manager",
                        "manager",
                        "manager",
                        Manager::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecObject::new(
                        "attachment",
                        "attachment",
                        "attachment",
                        crate::backend::Attachment::static_type(),
                        ParamFlags::READWRITE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "attachment" => self.attachment.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let man = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `Attachment` has to be of type `Manager`");
                    self.manager.replace(man);
                }
                "attachment" => {
                    let att = value
                        .get::<Option<crate::backend::Attachment>>()
                        .expect("Property `attachment` of `Attachment` has to be of type `crate::backend::Attachment`");
                    self.attachment.replace(att);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for Attachment {}
    impl BoxImpl for Attachment {}
}
