use std::time::Duration;

use gio::subclass::prelude::ObjectSubclassIsExt;
use gtk::prelude::*;
use gtk::SorterChange;
use gtk::{gio, glib};

use crate::backend::Manager;
use crate::{backend::Channel, gspawn};

glib::wrapper! {
    pub struct ChannelList(ObjectSubclass<imp::ChannelList>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl ChannelList {
    fn add_channel(&self, channel: Channel) {
        crate::trace!("`ChannelList` got new `Channel`: {}", channel.title());
        let obj = self.imp();
        obj.model.borrow().append(&channel);
        obj.sorter.borrow().changed(SorterChange::Different);
        self.scroll_up();
        let s = self.clone();
        channel.connect_notify_local(Some("last-message"), move |_, _| {
            log::trace!("Change sorter");
            s.imp().sorter.borrow().changed(SorterChange::Different);
            s.imp().filter_changed();
            s.scroll_up();
        });
    }

    pub fn scroll_up(&self) {
        gspawn!(glib::clone!(@strong self as s => async move  {
            // Need to sleep a little to make sure the scrolled window saw the changed
            // child.
            glib::timeout_future(Duration::from_millis(50)).await;
            let adjustment = s.imp().scrolled_window.vadjustment();
            adjustment.set_value(adjustment.lower());
        }));
    }

    pub fn activate_row(&self, i: u32) -> bool {
        let obj = self.imp();
        let model = obj
            .list
            .model()
            .expect("`ChannelList` list to have a model");
        if let Some(channel) = model.item(i).and_then(|c| c.downcast::<Channel>().ok()) {
            self.set_property("active-channel", channel);
            true
        } else {
            false
        }
    }

    pub fn toggle_search(&self) {
        let obj = self.imp();
        if self.search_enabled() {
            obj.search_entry.emit_stop_search();
        } else {
            self.set_search_enabled(true);
            obj.search_entry.grab_focus();
        }
    }

    pub fn search_enabled(&self) -> bool {
        self.property("search-enabled")
    }

    pub fn set_search_enabled(&self, enabled: bool) {
        self.set_property("search-enabled", enabled)
    }

    pub fn manager(&self) -> Manager {
        self.property("manager")
    }
}

pub mod imp {
    use std::cell::{Cell, RefCell};

    use glib::{
        clone,
        once_cell::sync::Lazy,
        subclass::{InitializingObject, Signal},
        ParamSpec, ParamSpecBoolean, ParamSpecObject, Value,
    };
    use gtk::{gio, glib, EveryFilter};
    use gtk::{
        prelude::*, subclass::prelude::*, CompositeTemplate, CustomFilter, CustomSorter,
        FilterChange, FilterListModel, SignalListItemFactory, SortListModel, Widget,
    };

    use crate::{
        backend::{timeline::timeline_item::TimelineItemExt, Channel, Manager},
        gui::{channel_item::ChannelItem, utility::Utility},
    };

    #[derive(CompositeTemplate)]
    #[template(resource = "/ui/channel_list.ui")]
    pub struct ChannelList {
        #[template_child]
        pub(super) scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub(super) list: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,

        pub(super) model: RefCell<gio::ListStore>,
        pub(super) sorter: RefCell<gtk::CustomSorter>,
        pub(super) filter: RefCell<gtk::EveryFilter>,

        manager: RefCell<Option<Manager>>,
        active_channel: RefCell<Option<Channel>>,
        search_enabled: Cell<bool>,
        add_conversation_enabled: Cell<bool>,
    }

    impl Default for ChannelList {
        fn default() -> Self {
            Self {
                scrolled_window: Default::default(),
                list: Default::default(),
                search_entry: Default::default(),
                model: RefCell::new(gio::ListStore::new::<Channel>()),
                sorter: Default::default(),
                filter: Default::default(),
                manager: Default::default(),
                active_channel: Default::default(),
                search_enabled: Default::default(),
                add_conversation_enabled: Default::default(),
            }
        }
    }

    #[gtk::template_callbacks]
    impl ChannelList {
        #[template_callback]
        fn search_changed(&self) {
            self.filter_changed();
            self.obj().scroll_up();
        }

        pub(super) fn filter_changed(&self) {
            self.filter.borrow().changed(FilterChange::Different);
        }

        #[template_callback]
        fn search_activate(&self) {
            let obj = self.obj();
            if obj.activate_row(0) {
                obj.toggle_search();
            }
        }

        #[template_callback]
        fn search_stopped(&self) {
            self.obj().set_search_enabled(false);
            self.search_entry.set_text("");
            self.filter.borrow().changed(FilterChange::Different);
            self.list.grab_focus();
            self.obj().scroll_up();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChannelList {
        const NAME: &'static str = "FlChannelList";
        type Type = super::ChannelList;
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

    impl ObjectImpl for ChannelList {
        fn constructed(&self) {
            let obj = self.obj();
            let model = gtk::gio::ListStore::new::<Channel>();
            let filter_search =
                CustomFilter::new(clone!(@strong self.search_entry as entry => move |obj| {
                    let search = entry.text().to_string();
                    let channel = obj
                        .downcast_ref::<Channel>()
                        .expect("The object needs to be of type `Channel`.");
                    let title = channel.title();
                    title.to_lowercase().contains(&search.to_lowercase())
                }));
            let filter = EveryFilter::new();
            let filter_empty = CustomFilter::new(clone!(@strong obj as o => move |obj| {
                let channel = obj
                    .downcast_ref::<Channel>()
                    .expect("The object needs to be of type `Channel`.");
                let has_message = channel.last_message().is_some();
                has_message || o.property("add-conversation-enabled")
            }));
            filter.append(filter_search);
            filter.append(filter_empty);
            let filter_model = FilterListModel::new(Some(model.clone()), Some(filter.clone()));
            let sorter = CustomSorter::new(|l1, l2| {
                let c1 = l1
                    .downcast_ref::<Channel>()
                    .expect("The object to be a channel");
                let c2 = l2
                    .downcast_ref::<Channel>()
                    .expect("The object to be a channel");

                let m1 = c1.last_message();
                let m2 = c2.last_message();

                if m1.is_some() && m2.is_none() {
                    return gtk::Ordering::Smaller;
                } else if m1.is_none() && m2.is_some() {
                    return gtk::Ordering::Larger;
                } else if let (Some(m1), Some(m2)) = (m1, m2) {
                    let s1 = m1.timestamp();
                    let s2 = m2.timestamp();
                    if s1 > s2 {
                        return gtk::Ordering::Smaller;
                    } else {
                        return gtk::Ordering::Larger;
                    }
                }

                if c1.title() < c2.title() {
                    gtk::Ordering::Smaller
                } else {
                    gtk::Ordering::Larger
                }
            });
            let sort_model = SortListModel::new(Some(filter_model), Some(sorter.clone()));

            let selection_model = gtk::NoSelection::new(Some(sort_model));
            self.list.get().set_model(Some(&selection_model));

            self.model.replace(model);
            self.sorter.replace(sorter);
            self.filter.replace(filter);

            let factory = SignalListItemFactory::new();
            factory.connect_setup(move |_, object| {
                let list_item = object.downcast_ref::<gtk::ListItem>().unwrap();
                let channel_item = ChannelItem::new();
                list_item.set_child(Some(&channel_item));

                list_item
                    .property_expression("item")
                    .bind(&channel_item, "channel", Widget::NONE);
            });
            self.list.set_factory(Some(&factory));
            self.list.set_single_click_activate(true);

            self.list
                .connect_activate(clone!(@weak self as obj => move |_list_view, position| {
                    obj.obj().activate_row(position);
                }));

            obj.connect_notify_local(
                Some("add-conversation-enabled"),
                clone!(@weak self as s => move |_, _| {
                    s.filter_changed();
                }),
            );
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<Manager>("manager").build(),
                    ParamSpecObject::builder::<Channel>("active-channel").build(),
                    ParamSpecBoolean::builder("search-enabled").build(),
                    ParamSpecBoolean::builder("add-conversation-enabled").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "active-channel" => self.active_channel.borrow().as_ref().to_value(),
                "search-enabled" => self.search_enabled.get().to_value(),
                "add-conversation-enabled" => self.add_conversation_enabled.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let man = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `ChannelList` has to be of type `Manager`");

                    if let Some(man) = &man {
                        log::trace!(
                            "Connecting to the `channel` signal of `Manager` in `ChannelList`"
                        );
                        man.connect_local(
                            "channel",
                            false,
                            clone!(@weak self as obj => @default-return None, move |args| {
                                let channel = args[1]
                                    .get::<Channel>()
                                    .expect("Type of `channel` signal of `Manager` to be `Channel`");
                                obj.obj().add_channel(channel);
                                None
                            }),
                        );
                    }
                    self.manager.replace(man);
                }
                "active-channel" => {
                    let chan = value.get::<Option<Channel>>().expect(
                        "Property `active-channel` of `ChannelList` has to be of type `Channel`",
                    );
                    self.obj()
                        .emit_by_name::<()>("active-channel-changed", &[&chan]);
                    self.active_channel.replace(chan);
                }
                "search-enabled" => {
                    let search = value.get::<bool>().expect(
                        "Property `search-enabled` of `ChannelList` has to be of type `bool`",
                    );
                    self.search_enabled.replace(search);
                }
                "add-conversation-enabled" => {
                    let add = value.get::<bool>().expect(
                        "Property `add-conversation-enabled` of `ChannelList` has to be of type `bool`",
                    );
                    self.add_conversation_enabled.replace(add);
                }
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| -> Vec<Signal> {
                vec![Signal::builder("active-channel-changed")
                    .param_types([Channel::static_type()])
                    .build()]
            });
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for ChannelList {}
    impl BoxImpl for ChannelList {}
}
