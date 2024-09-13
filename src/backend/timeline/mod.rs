pub mod timeline_item;

use crate::prelude::*;

pub use timeline_item::*;

const TRIM_SIZE: usize = 1;

glib::wrapper! {
    /// The timeline stores ordered [TimelineItem]s (i.e. [Message](crate::backend::Message)s)
    pub struct Timeline(ObjectSubclass<imp::Timeline>)
        @implements gio::ListModel, gtk::SectionModel;
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Timeline {
    pub fn new() -> Timeline {
        Object::builder().build()
    }

    /// Remove old messages to free memory.
    pub fn trim_old(&self) {
        let mut current_items = self.imp().list.borrow_mut();
        if current_items.len() > TRIM_SIZE {
            let to_trim = current_items.len() - TRIM_SIZE;
            let new = current_items.split_off(to_trim);
            drop(current_items);
            self.imp().list.replace(new);
            self.items_changed(0, to_trim as u32, 0);
        }
    }

    /// Completely clear the list.
    pub fn clear(&self) {
        let mut list = self.imp().list.borrow_mut();
        let len = list.len();
        list.clear();
        drop(list);
        self.items_changed(0, len as u32, 0);
    }

    pub fn get_by_timestamp(&self, timestamp: u64) -> Option<TimelineItem> {
        let current_items = self.imp().list.borrow();
        let index = current_items.binary_search_by_key(&timestamp, |i| i.timestamp());
        match index {
            Ok(i) => current_items.get(i).cloned(),
            Err(_) => None,
        }
    }

    pub fn iter_forwards(&self) -> impl Iterator<Item = TimelineItem> + 'static {
        let current_items = self.imp().list.borrow();
        current_items.clone().into_iter()
    }

    pub fn iter_backwards(&self) -> impl Iterator<Item = TimelineItem> + 'static {
        let current_items = self.imp().list.borrow();
        current_items.clone().into_iter().rev()
    }

    fn items_changed(&self, position: u32, removed: u32, added: u32) {
        self.update_show_header(position, removed, added);
        self.update_show_timestamp(position, removed, added);
        self.upcast_ref::<gio::ListModel>()
            .items_changed(position, removed, added);
    }

    fn update_show_header(&self, position: u32, _removed: u32, added: u32) {
        let current_items = self.imp().list.borrow();
        if position == 0 {
            // The start was modified.
            let items_iter = current_items.iter();
            let mut previous = None;

            // Iterate over all the added items, plus one as the next may also require an update
            for i in items_iter.take(added as usize + 1) {
                i.update_show_header(previous);
                previous = Some(i);
            }
        } else {
            // Somewhere else was modified. Skip to the position (minus one to get the previous
            // item for the first one to calculate).
            let mut items_iter = current_items.iter().skip(position as usize - 1);
            let mut previous = items_iter.next();

            // Iterate over all the added items, plus one as the next may also require an update
            for i in items_iter.take(added as usize + 1) {
                i.update_show_header(previous);
                previous = Some(i);
            }
        }
    }

    fn update_show_timestamp(&self, position: u32, _removed: u32, added: u32) {
        let current_items = self.imp().list.borrow();
        if position + added == current_items.len() as u32 {
            // The end was modified.
            let items_iter = current_items.iter().rev();
            let mut next = None;

            // Iterate over all the added items (in reverse order), plus one as the previous may also require an update
            for i in items_iter.take(added as usize + 1) {
                i.update_show_timestamp(next);
                next = Some(i);
            }
        } else {
            // Somewhere else was modified. Skip to the position (minus one to get the previous
            // item for the first one to calculate).
            let mut items_iter = current_items
                .iter()
                .rev()
                .skip(current_items.len() - position as usize - added as usize - 1);
            let mut next = items_iter.next();

            // Iterate over all the added items (in reverse), plus one as the next may also require an update
            for i in items_iter.take(added as usize + 1) {
                i.update_show_timestamp(next);
                next = Some(i);
            }
        }
    }

    /// Add items to the timeline that are expected to be prepended. Note that this may also add "in
    /// between" depending on the timestamps. Expects the item to be sorted by timestamp increasing.
    ///
    /// If there already exists a item with the same timestamp like in the list, the old item will
    /// be overwritten with the new one.
    pub fn prepend(&self, items: Vec<TimelineItem>) {
        if items.is_empty() {
            return;
        }
        let number_of_items = items.len() as u32;

        let mut current_items = self.imp().list.borrow_mut();
        current_items.reserve(items.len());

        if current_items.is_empty() {
            current_items.extend(items);
            drop(current_items);
            self.items_changed(0, 0, number_of_items);
            return;
        }

        let mut front_ts = current_items.front().unwrap().timestamp();

        // Happy path: Just insert the entire list at the front.
        if front_ts > items.last().unwrap().timestamp() {
            for item in items.into_iter().rev() {
                current_items.push_front(item);
            }
            drop(current_items);
            self.items_changed(0, 0, number_of_items);
            return;
        }

        drop(current_items);

        for item in items.into_iter().rev() {
            if item.timestamp() < front_ts {
                // Happy path: Can just insert at the front.
                front_ts = item.timestamp();
                let mut current_items = self.imp().list.borrow_mut();
                current_items.push_front(item);
                drop(current_items);
                self.items_changed(0, 0, 1);
            } else {
                // Unhappy path: Search for the required index to insert.
                let mut current_items = self.imp().list.borrow_mut();
                let to_insert =
                    current_items.binary_search_by_key(&item.timestamp(), |i| i.timestamp());
                match to_insert {
                    // Already included. Remove the existing and add the new.
                    Ok(idx) => {
                        current_items.remove(idx);
                        current_items.insert(idx, item);
                        drop(current_items);
                        self.items_changed(idx as u32, 1, 1);
                    }
                    // Not existing.
                    Err(idx) => {
                        current_items.insert(idx, item);
                        drop(current_items);
                        self.items_changed(idx as u32, 0, 1);
                    }
                }
            }
        }
    }

    /// Insert a timeline item that is probably at the back of the list.
    pub fn append(&self, item: TimelineItem) {
        let mut current_items = self.imp().list.borrow_mut();
        let number_of_items = current_items.len() as u32;

        // Happy path: Either the list is empty or the last item has smaller timestamp.
        if current_items
            .back()
            .map(|l| l.timestamp())
            .unwrap_or_default()
            < item.timestamp()
        {
            current_items.push_back(item);
            drop(current_items);
            self.items_changed(number_of_items, 0, 1);
        } else {
            let to_insert =
                current_items.binary_search_by_key(&item.timestamp(), |i| i.timestamp());
            match to_insert {
                // Already included. Remove the existing and add the new.
                Ok(idx) => {
                    current_items.remove(idx);
                    current_items.insert(idx, item);
                    drop(current_items);
                    self.items_changed(idx as u32, 1, 1);
                }
                // Not existing.
                Err(idx) => {
                    current_items.insert(idx, item);
                    drop(current_items);
                    self.items_changed(idx as u32, 0, 1);
                }
            }
        }
    }
}

mod imp {
    use crate::prelude::*;
    use std::collections::VecDeque;

    use super::{TimelineItem, TimelineItemExt};

    #[derive(Debug, Default)]
    pub struct Timeline {
        pub list: RefCell<VecDeque<TimelineItem>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Timeline {
        const NAME: &'static str = "Timeline";
        type Type = super::Timeline;
        type Interfaces = (gio::ListModel, gtk::SectionModel);
    }

    impl ObjectImpl for Timeline {}

    impl ListModelImpl for Timeline {
        fn item_type(&self) -> glib::Type {
            TimelineItem::static_type()
        }

        fn n_items(&self) -> u32 {
            self.list.borrow().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            let list = self.list.borrow();

            list.get(position as usize)
                .map(|o| o.clone().upcast::<glib::Object>())
        }
    }

    impl SectionModelImpl for Timeline {
        fn section(&self, pos: u32) -> (u32, u32) {
            let list = self.list.borrow();

            // As per the doc: <https://docs.gtk.org/gtk4/method.SectionModel.get_section.html>
            // > If the position is larger than the number of items, a single range from n_items to G_MAXUINT will be returned.
            if pos >= TryInto::<u32>::try_into(list.len()).unwrap_or_default() {
                return (
                    TryInto::<u32>::try_into(list.len()).unwrap_or_default(),
                    u32::MAX,
                );
            }

            // Unwrap should never happen due to previous check.
            let day = list
                .get(pos.try_into().unwrap_or_default())
                .map(|t| t.day_timestamp())
                .unwrap_or_default();

            // Scan forward
            let first_in = list.partition_point(|t| t.day_timestamp() < day);
            let first_out = list.partition_point(|t| t.day_timestamp() < day + 1);

            (
                first_in.try_into().unwrap_or_default(),
                first_out.try_into().unwrap_or_default(),
            )
        }
    }
}
