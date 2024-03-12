use crate::backend::Channel;
use adw::prelude::AdwDialogExt;
use gdk::{
    gio::{self, prelude::SettingsExt},
    glib::{object::ObjectExt, subclass::types::ObjectSubclassIsExt},
};
use gtk::glib;

use super::Window;

glib::wrapper! {
    pub struct NewChannelDialog(ObjectSubclass<imp::NewChannelDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl NewChannelDialog {
    pub fn present_for_selection(&self, channels: &[Channel]) {
        self.imp().set_channels(channels);
        self.present(&self.property::<Window>("window"));
    }

    fn settings(&self) -> gio::Settings {
        self.property::<Window>("window").settings()
    }

    fn sort_by(&self) -> String {
        self.settings().string("sort-contacts-by").to_string()
    }

    fn set_sort_by(&self, s: &str) {
        let imp = self.imp();
        let _ = self.settings().set_string("sort-contacts-by", s);
        imp.set_channels(&imp.channels.replace(vec![]));
    }
}

pub mod imp {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::time::Duration;

    use adw::prelude::AdwDialogExt;
    use adw::subclass::prelude::*;
    use gdk::gio;
    use gdk::gio::prelude::ListModelExt;
    use gdk::glib::clone;
    use gdk::glib::object::Cast;
    use gdk::glib::object::ObjectExt;
    use gdk::glib::subclass::Signal;
    use gdk::glib::types::StaticType;
    use gdk::glib::value::ToValue;
    use gdk::glib::variant::ToVariant;
    use gdk::glib::ParamSpec;
    use gdk::glib::ParamSpecBuilderExt;
    use gdk::glib::ParamSpecObject;
    use gdk::glib::Value;
    use gdk::glib::Variant;
    use gdk::glib::VariantTy;
    use gio::{SimpleAction, SimpleActionGroup};
    use glib::subclass::InitializingObject;
    use glib::Propagation;
    use gtk::glib;
    use gtk::prelude::ActionMapExt;
    use gtk::prelude::AdjustmentExt;
    use gtk::prelude::EditableExt;
    use gtk::prelude::FilterExt;
    use gtk::prelude::GObjectPropertyExpressionExt;
    use gtk::prelude::ListItemExt;
    use gtk::prelude::WidgetExt;
    use gtk::CompositeTemplate;
    use gtk::CustomFilter;
    use gtk::FilterChange;
    use gtk::FilterListModel;
    use gtk::NoSelection;
    use gtk::SignalListItemFactory;
    use gtk::Widget;
    use once_cell::sync::Lazy;

    use crate::backend::Channel;
    use crate::gspawn;
    use crate::gui::channel_item_compact::ChannelItemCompact;
    use crate::gui::components::letter_divider::LetterDivider;
    use crate::gui::utility::Utility;
    use crate::gui::Window;

    #[derive(CompositeTemplate)]
    #[template(resource = "/ui/new_channel_dialog.ui")]
    pub struct NewChannelDialog {
        #[template_child]
        list_channels: TemplateChild<gtk::ListView>,
        #[template_child]
        scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        search_entry: TemplateChild<gtk::SearchEntry>,

        pub(super) channels: RefCell<Vec<Channel>>,

        model: RefCell<gtk::FlattenListModel>,
        filter: RefCell<gtk::CustomFilter>,

        window: RefCell<Option<Window>>,
    }

    impl Default for NewChannelDialog {
        fn default() -> Self {
            Self {
                list_channels: Default::default(),
                scrolled_window: Default::default(),
                search_entry: Default::default(),
                channels: RefCell::new(vec![]),
                model: RefCell::new(gtk::FlattenListModel::new(None::<gio::ListStore>)),
                filter: Default::default(),
                window: Default::default(),
            }
        }
    }

    #[gtk::template_callbacks]
    impl NewChannelDialog {
        #[template_callback]
        fn search_changed(&self) {
            self.filter_changed();
            self.scroll_up();
        }

        #[template_callback]
        fn search_activated(&self) {
            self.activate_row(0);
        }

        fn filter_changed(&self) {
            self.filter.borrow().changed(FilterChange::Different);
        }

        fn scroll_up(&self) {
            let obj = self.obj();
            gspawn!(glib::clone!(@strong obj => async move  {
                // Need to sleep a little to make sure the scrolled window saw the changed
                // child.
                glib::timeout_future(Duration::from_millis(50)).await;
                let adjustment = obj.imp().scrolled_window.vadjustment();
                adjustment.set_value(adjustment.lower());
            }));
        }

        /// By default, the search entry swallows the escape key. Close the popup instead like done everywhere else on escape.
        fn connect_quit_on_escape(&self) {
            let obj = self.obj();
            let key_events = gtk::EventControllerKey::new();
            key_events.connect_key_pressed(
                clone!(@weak obj => @default-return Propagation::Proceed, move |_, key, _, _| {
                    if key == gdk::Key::Escape {
                        obj.close();
                        Propagation::Stop
                    } else {
                        Propagation::Proceed
                    }
                }),
            );
            self.search_entry.add_controller(key_events);
        }

        fn build_section_model_from_channels(channels: &[Channel]) -> gio::ListStore {
            if channels.is_empty() {
                return gio::ListStore::new::<gio::ListStore>();
            }

            let mut channels = channels.iter().collect::<VecDeque<_>>();
            channels
                .make_contiguous()
                .sort_by_key(|c| c.sort_name().to_lowercase());

            let result = gio::ListStore::new::<gio::ListStore>();
            let mut current_model = gio::ListStore::new::<Channel>();
            result.append(&current_model);
            let mut current_letter = channels[0]
                .sort_name()
                .chars()
                .next()
                .unwrap_or_default()
                .to_lowercase()
                .to_string();

            while let Some(channel) = channels.pop_front() {
                let letter = channel
                    .sort_name()
                    .chars()
                    .next()
                    .unwrap_or_default()
                    .to_lowercase()
                    .to_string();
                if current_letter != letter {
                    current_model = gio::ListStore::new::<Channel>();
                    result.append(&current_model);
                }
                current_letter = letter;
                current_model.append(channel);
            }

            result
        }

        pub(super) fn set_channels(&self, channels: &[Channel]) {
            let model = self.model.borrow();
            model.set_model(Some(&Self::build_section_model_from_channels(channels)));
            self.channels.replace(channels.to_vec());

            self.search_entry.set_text("");
            self.scroll_up();
        }

        fn activate_row(&self, i: u32) {
            let obj = self.obj();
            let model = self
                .list_channels
                .model()
                .expect("`NewChannelDialog` list to have a model");
            if let Some(channel) = model.item(i).and_then(|c| c.downcast::<Channel>().ok()) {
                obj.emit_by_name::<()>("channel", &[&channel]);
                obj.close();
            }
        }

        fn setup_model(&self) {
            let model = self.model.borrow();

            let filter_search =
                CustomFilter::new(clone!(@strong self.search_entry as entry => move |obj| {
                    let search = entry.text().to_string();
                    let channel = obj
                        .downcast_ref::<Channel>()
                        .expect("The object needs to be of type `Channel`.");
                    let title = channel.title();
                    title.to_lowercase().contains(&search.to_lowercase())
                }));
            let filter_model =
                FilterListModel::new(Some(model.clone()), Some(filter_search.clone()));

            self.filter.replace(filter_search);

            let factory = SignalListItemFactory::new();
            factory.connect_setup(move |_, object| {
                let list_item = object.downcast_ref::<gtk::ListItem>().unwrap();
                let channel_item = ChannelItemCompact::new();
                list_item.set_child(Some(&channel_item));
                list_item
                    .property_expression("item")
                    .bind(&channel_item, "channel", Widget::NONE);
            });

            let header_factory = SignalListItemFactory::new();
            header_factory.connect_setup(|_, object| {
                let widget = LetterDivider::default();
                let header_item = object.downcast_ref::<gtk::ListHeader>().unwrap();
                header_item.set_child(Some(&widget));
                header_item
                    .bind_property("item", &widget, "string")
                    .transform_to(|_, c: Option<Channel>| c.map(|c| c.sort_name()))
                    .build();
            });

            self.list_channels.set_factory(Some(&factory));
            self.list_channels.set_header_factory(Some(&header_factory));
            self.list_channels.set_single_click_activate(true);
            self.list_channels
                .set_model(Some(&NoSelection::new(Some(filter_model))));

            self.list_channels.connect_activate(
                clone!(@weak self as s => move |_list_view, position| {
                    s.activate_row(position);
                }),
            );
        }

        fn setup_actions(&self) {
            let obj = self.obj();
            let action_sort_on = SimpleAction::new_stateful(
                "sort-on",
                Some(VariantTy::STRING),
                &obj.sort_by().to_variant(),
            );
            action_sort_on.connect_activate(clone!(@weak obj => move |action, target| {
                let Some(target) = target.and_then(|t| t.str()) else {
                    log::error!("NewChannelDialog action `sort-on` got invalid variant type");
                    return;
                };
                action.set_state(&Variant::from(target));
                obj.set_sort_by(target);
                obj.imp().filter_changed();
            }));

            let actions = SimpleActionGroup::new();
            obj.insert_action_group("new-chat-dialog", Some(&actions));
            actions.add_action(&action_sort_on);
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NewChannelDialog {
        const NAME: &'static str = "FlNewChannelDialog";
        type Type = super::NewChannelDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for NewChannelDialog {
        fn constructed(&self) {
            self.setup_model();
            self.setup_actions();
            self.connect_quit_on_escape();
            self.parent_constructed();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![ParamSpecObject::builder::<Window>("window")
                    .construct_only()
                    .build()]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "window" => self.window.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "window" => {
                    let win = value.get::<Option<Window>>().expect(
                        "Property `window` of `NewChannelDialog` has to be of type `Window`",
                    );

                    self.window.replace(win);
                }
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| -> Vec<Signal> {
                vec![Signal::builder("channel")
                    .param_types([Channel::static_type()])
                    .build()]
            });
            SIGNALS.as_ref()
        }
    }
    impl WidgetImpl for NewChannelDialog {}
    impl AdwDialogImpl for NewChannelDialog {}
}
