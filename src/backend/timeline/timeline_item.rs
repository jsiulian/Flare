use crate::prelude::*;
use glib::subclass::types::IsSubclassable;

glib::wrapper! {
    /// A [TimelineItem] is anything that can be included in a [Timeline](super::Timeline).
    pub struct TimelineItem(ObjectSubclass<imp::TimelineItem>);
}

pub trait TimelineItemExt: 'static + std::marker::Sized + glib::prelude::ObjectExt {
    /// Set if this item should show the header based on the previous item.
    fn update_show_header(&self, previous: Option<&TimelineItem>);
    /// Set if this item should show the timestamp based on the next item.
    fn update_show_timestamp(&self, next: Option<&TimelineItem>);

    /// Days since 01.01.1970
    fn day_timestamp(&self) -> u64 {
        self.timestamp() / (1000 * 60 * 60 * 24)
    }

    /// The timestamp in ms from 01.01.1970
    fn timestamp(&self) -> u64 {
        self.property("timestamp")
    }

    fn show_header(&self) -> bool {
        self.property("show-header")
    }

    fn set_show_header(&self, value: bool) {
        self.set_property("show-header", value)
    }

    fn show_timestamp(&self) -> bool {
        self.property("show-timestamp")
    }

    fn set_show_timestamp(&self, value: bool) {
        self.set_property("show-timestamp", value)
    }
}

impl<O: IsA<TimelineItem>> TimelineItemExt for O {
    fn update_show_header(&self, previous: Option<&TimelineItem>) {
        imp::timeline_item_update_show_header(self.upcast_ref(), previous)
    }

    fn update_show_timestamp(&self, previous: Option<&TimelineItem>) {
        imp::timeline_item_update_show_timestamp(self.upcast_ref(), previous)
    }
}

pub trait TimelineItemImpl: ObjectImpl {
    fn update_show_header(&self, _obj: &Self::Type, _previous: Option<&TimelineItem>) {}
    fn update_show_timestamp(&self, _obj: &Self::Type, _previous: Option<&TimelineItem>) {}
}

unsafe impl<T> IsSubclassable<T> for TimelineItem
where
    T: TimelineItemImpl,
    T::Type: IsA<TimelineItem>,
{
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class.upcast_ref_mut());

        let klass = class.as_mut();

        klass.update_show_header = update_show_header_trampoline::<T>;
        klass.update_show_timestamp = update_show_timestamp_trampoline::<T>;
    }
}

fn update_show_header_trampoline<T>(this: &TimelineItem, previous: Option<&TimelineItem>)
where
    T: ObjectSubclass + TimelineItemImpl,
    T::Type: IsA<TimelineItem>,
{
    let this = this.downcast_ref::<T::Type>().unwrap();
    this.imp().update_show_header(this, previous)
}

fn update_show_timestamp_trampoline<T>(this: &TimelineItem, next: Option<&TimelineItem>)
where
    T: ObjectSubclass + TimelineItemImpl,
    T::Type: IsA<TimelineItem>,
{
    let this = this.downcast_ref::<T::Type>().unwrap();
    this.imp().update_show_timestamp(this, next)
}

mod imp {
    use std::marker::PhantomData;

    use super::*;

    #[repr(C)]
    pub struct TimelineItemClass {
        pub parent_class: glib::object::ObjectClass,
        pub update_show_header: fn(&super::TimelineItem, Option<&super::TimelineItem>),
        pub update_show_timestamp: fn(&super::TimelineItem, Option<&super::TimelineItem>),
    }

    unsafe impl ClassStruct for TimelineItemClass {
        type Type = TimelineItem;
    }

    pub(super) fn timeline_item_update_show_header(
        this: &super::TimelineItem,
        previous: Option<&super::TimelineItem>,
    ) {
        let klass = this.class();
        (klass.as_ref().update_show_header)(this, previous)
    }

    pub(super) fn timeline_item_update_show_timestamp(
        this: &super::TimelineItem,
        next: Option<&super::TimelineItem>,
    ) {
        let klass = this.class();
        (klass.as_ref().update_show_timestamp)(this, next)
    }

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::TimelineItem)]
    pub struct TimelineItem {
        #[property(get, set, construct_only)]
        timestamp: Cell<u64>,

        #[property(get, set)]
        show_header: Cell<bool>,
        #[property(get, set)]
        show_timestamp: Cell<bool>,

        #[property(get = Self::datetime)]
        datetime: PhantomData<Option<glib::DateTime>>,
    }

    impl TimelineItem {
        // Time as [glib::DateTime]. Should not return `None`, but just in case.
        fn datetime(&self) -> Option<glib::DateTime> {
            glib::DateTime::from_unix_utc(
                (self.timestamp.get() / 1000).try_into().unwrap_or_default(),
            )
            .ok()
            .and_then(|d| d.to_local().ok())
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TimelineItem {
        const NAME: &'static str = "FlTimelineItem";
        type Type = super::TimelineItem;
        type ParentType = glib::Object;
        type Class = TimelineItemClass;
    }

    #[glib::derived_properties]
    impl ObjectImpl for TimelineItem {}
}
