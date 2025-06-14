use crate::prelude::*;

use std::time::Duration;

use gtk::SorterChange;

use crate::backend::Channel;

glib::wrapper! {
    /// The list of channels.
    pub struct ChannelList(ObjectSubclass<imp::ChannelList>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl ChannelList {
    pub fn add_channel(&self, channel: Channel) {
        crate::trace!("`ChannelList` got new `Channel`: {}", channel.title());
        let obj = self.imp();
        let model = obj.model.borrow();

        if let Some(pos) = model.find(&channel) {
            crate::trace!(
                "`ChannelList` got duplicated `Channel`: {}. Just setting as active channel",
                channel.title()
            );
            self.imp()
                .list
                .scroll_to(pos, gtk::ListScrollFlags::NONE, None);
            self.imp()
                .selection_model
                .borrow()
                .set_selected_chat(Some(&channel));
            self.set_active_channel(Some(channel));
            return;
        }

        model.append(&channel);
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
        gspawn!(glib::clone!(
            #[strong(rename_to = s)]
            self,
            async move {
                // Need to sleep a little to make sure the scrolled window saw the changed
                // child.
                glib::timeout_future(Duration::from_millis(50)).await;
                let adjustment = s.imp().scrolled_window.vadjustment();
                adjustment.set_value(adjustment.lower());
            }
        ));
    }

    pub fn activate_row(&self, i: u32) -> bool {
        let obj = self.imp();
        let model = obj
            .list
            .model()
            .expect("`ChannelList` list to have a model");
        if let Some(channel) = model.item(i).and_then(|c| c.downcast::<Channel>().ok()) {
            if let Some(previously_active) = self.active_channel() {
                previously_active.set_active(false);
            }
            self.set_active_channel(Some(channel));
            self.set_active(true);
            true
        } else {
            false
        }
    }

    pub fn set_active(&self, active: bool) {
        if let Some(channel) = self.active_channel() {
            channel.set_active(active);
            self.withdraw_notifications();
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

    fn withdraw_notifications(&self) {
        let Some(channel) = self.active_channel() else {
            return;
        };
        let Some(application) = self.manager().application() else {
            return;
        };
        for uid in channel.mark_as_read() {
            application.withdraw_notification(&uid);
        }
    }
}

pub mod imp {
    use crate::prelude::*;

    use glib::subclass::InitializingObject;
    use gtk::{
        CompositeTemplate, CustomFilter, CustomSorter, EveryFilter, FilterChange, FilterListModel,
        SignalListItemFactory, SortListModel, Widget,
    };

    use crate::{
        backend::{Channel, timeline::timeline_item::TimelineItemExt},
        gui::{channel_item::ChannelItem, components::Selection},
    };

    #[derive(CompositeTemplate, glib::Properties)]
    #[template(resource = "/ui/channel_list.ui")]
    #[properties(wrapper_type = super::ChannelList)]
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
        pub(super) selection_model: RefCell<Selection>,

        #[property(get, set = Self::set_manager, type = Manager)]
        manager: RefCell<Option<Manager>>,
        #[property(get, set, nullable)]
        active_channel: RefCell<Option<Channel>>,
        #[property(get, set)]
        search_enabled: Cell<bool>,
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
                selection_model: RefCell::new(Selection::new(
                    gio::ListStore::new::<Channel>().into(),
                )),
                manager: Default::default(),
                active_channel: Default::default(),
                search_enabled: Default::default(),
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

        fn set_manager(&self, manager: Option<Manager>) {
            if let Some(manager) = &manager {
                log::trace!("Connecting to the `channel` signal of `Manager` in `ChannelList`");
                manager.connect_local(
                    "channel",
                    false,
                    clone!(
                        #[weak(rename_to = s)]
                        self,
                        #[upgrade_or_default]
                        move |args| {
                            let channel = args[1]
                                .get::<Channel>()
                                .expect("Type of `channel` signal of `Manager` to be `Channel`");
                            s.obj().add_channel(channel);
                            None
                        }
                    ),
                );
            }
            self.manager.replace(manager);
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
            Selection::ensure_type();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ChannelList {
        fn constructed(&self) {
            let obj = self.obj();
            let model = gtk::gio::ListStore::new::<Channel>();

            // Filter
            let filter_search = CustomFilter::new(clone!(
                #[strong(rename_to = entry)]
                self.search_entry,
                move |obj| {
                    let search = entry.text().to_string();
                    let channel = obj
                        .downcast_ref::<Channel>()
                        .expect("The object needs to be of type `Channel`.");
                    let title = channel.title();
                    title.to_lowercase().contains(&search.to_lowercase())
                }
            ));
            let filter = EveryFilter::new();
            let filter_empty = CustomFilter::new(clone!(
                #[strong(rename_to = o)]
                obj,
                move |obj| {
                    let channel = obj
                        .downcast_ref::<Channel>()
                        .expect("The object needs to be of type `Channel`.");
                    let has_message = channel.last_message().is_some();
                    let is_selected = Some(channel) == o.active_channel().as_ref();
                    has_message || is_selected
                }
            ));
            filter.append(filter_search);
            filter.append(filter_empty);
            let filter_model = FilterListModel::new(Some(model.clone()), Some(filter.clone()));

            // Sorter
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
                    return gtk::Ordering::Larger;
                }
                if m1.is_none() && m2.is_some() {
                    return gtk::Ordering::Smaller;
                }
                if let (Some(m1), Some(m2)) = (m1, m2) {
                    let s1 = m1.timestamp();
                    let s2 = m2.timestamp();
                    if s1 > s2 {
                        return gtk::Ordering::Smaller;
                    }
                    return gtk::Ordering::Larger;
                }

                if c1.title() < c2.title() {
                    gtk::Ordering::Smaller
                } else {
                    gtk::Ordering::Larger
                }
            });
            let sort_model = SortListModel::new(Some(filter_model), Some(sorter.clone()));

            // Selection
            let selection_model = Selection::new(sort_model.into());
            self.list.get().set_model(Some(&selection_model));

            self.model.replace(model);
            self.sorter.replace(sorter);
            self.filter.replace(filter);
            self.selection_model.replace(selection_model.clone());

            // Item factory.
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

            // Activate on click.
            self.list.connect_activate(clone!(
                #[weak(rename_to = s)]
                self,
                move |_list_view, position| {
                    s.obj().activate_row(position);
                    selection_model.set_selected_position(position);
                }
            ));
        }
    }

    impl WidgetImpl for ChannelList {}
    impl BoxImpl for ChannelList {}
}
