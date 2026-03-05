use std::cell::RefCell;

use cacao::appkit::window::{Window, WindowConfig, WindowDelegate, WindowStyle};
use cacao::geometry::Rect;
use cacao::layout::{Layout, LayoutConstraint};
use cacao::listview::{ListView, ListViewDelegate, ListViewRow};
use cacao::text::{Font, Label, LineBreakMode};
use cacao::view::{View, ViewDelegate};

const DEVICE_ROW: &str = "DeviceRowCell";

#[derive(Debug, Clone)]
pub struct DeviceEntry {
    pub name: String,
    pub id: u32,
    pub last_seen: String,
}

#[derive(Default, Debug)]
pub struct DeviceRow {
    name: Label,
    detail: Label,
}

impl ViewDelegate for DeviceRow {
    const NAME: &'static str = "DeviceRow";

    fn did_load(&mut self, view: View) {
        view.add_subview(&self.name);
        view.add_subview(&self.detail);

        self.name.set_font(&Font::bold_system(13.));
        self.name.set_line_break_mode(LineBreakMode::TruncateTail);
        self.detail.set_font(&Font::system(11.));
        use cacao::color::Color;
        self.detail.set_text_color(Color::SystemGray);

        LayoutConstraint::activate(&[
            self.name.top.constraint_equal_to(&view.top).offset(6.),
            self.name.leading.constraint_equal_to(&view.leading).offset(12.),
            self.name.trailing.constraint_equal_to(&view.trailing).offset(-12.),
            self.detail.top.constraint_equal_to(&self.name.bottom).offset(2.),
            self.detail.leading.constraint_equal_to(&view.leading).offset(12.),
            self.detail.trailing.constraint_equal_to(&view.trailing).offset(-12.),
            self.detail.bottom.constraint_equal_to(&view.bottom).offset(-6.),
        ]);
    }
}

impl DeviceRow {
    fn configure(&mut self, entry: &DeviceEntry) {
        let display_name = if entry.name.is_empty() {
            format!("Device {}", entry.id)
        } else {
            entry.name.clone()
        };
        self.name.set_text(&display_name);
        self.detail.set_text(&format!("Last seen: {}", entry.last_seen));
    }
}

#[derive(Debug, Default)]
pub struct DeviceListDelegate {
    view: Option<ListView>,
    devices: RefCell<Vec<DeviceEntry>>,
}

impl DeviceListDelegate {
    pub fn set_devices(&self, devices: Vec<DeviceEntry>) {
        *self.devices.borrow_mut() = devices;
        if let Some(v) = &self.view {
            v.reload();
        }
    }
}

impl ListViewDelegate for DeviceListDelegate {
    const NAME: &'static str = "DeviceListView";

    fn did_load(&mut self, view: ListView) {
        view.register(DEVICE_ROW, DeviceRow::default);
        view.set_uses_automatic_row_heights(true);
        view.set_row_height(50.);
        self.view = Some(view);
    }

    fn number_of_items(&self) -> usize {
        self.devices.borrow().len()
    }

    fn item_for(&self, row: usize) -> ListViewRow {
        let mut view = self.view.as_ref().unwrap().dequeue::<DeviceRow>(DEVICE_ROW);
        if let Some(delegate) = &mut view.delegate {
            let devices = self.devices.borrow();
            if let Some(entry) = devices.get(row) {
                delegate.configure(entry);
            }
        }
        view.into_row()
    }
}

pub struct LinkedDevicesDelegate {
    content: View,
    status_label: Label,
    device_list: ListView<DeviceListDelegate>,
}

impl Default for LinkedDevicesDelegate {
    fn default() -> Self {
        Self {
            content: View::new(),
            status_label: Label::new(),
            device_list: ListView::with(DeviceListDelegate::default()),
        }
    }
}

impl WindowDelegate for LinkedDevicesDelegate {
    const NAME: &'static str = "LinkedDevicesWindow";

    fn did_load(&mut self, window: Window) {
        window.set_title("Linked Devices");
        window.set_minimum_content_size(400.0, 300.0);
        window.set_titlebar_appears_transparent(true);

        self.status_label.set_text("Loading devices...");
        use cacao::color::Color;
        self.status_label.set_text_color(Color::SystemGray);

        self.content.add_subview(&self.status_label);
        self.content.add_subview(&self.device_list);

        LayoutConstraint::activate(&[
            self.status_label.center_x.constraint_equal_to(&self.content.center_x),
            self.status_label.center_y.constraint_equal_to(&self.content.center_y),
            self.device_list.leading.constraint_equal_to(&self.content.leading),
            self.device_list.trailing.constraint_equal_to(&self.content.trailing),
            self.device_list.top.constraint_equal_to(&self.content.top),
            self.device_list.bottom.constraint_equal_to(&self.content.bottom),
        ]);

        self.device_list.set_hidden(true);
        window.set_content_view(&self.content);
    }
}

pub struct LinkedDevicesWindow(pub Option<Window<LinkedDevicesDelegate>>);

impl LinkedDevicesWindow {
    pub fn show(&self) {
        if let Some(ref w) = self.0 {
            w.show();
        }
    }

    pub fn set_devices(&self, devices: Vec<DeviceEntry>) {
        if let Some(ref w) = self.0 {
            let d = w.delegate.as_ref().unwrap();
            d.status_label.set_hidden(true);
            d.device_list.set_hidden(false);
            if let Some(ref delegate) = d.device_list.delegate {
                delegate.set_devices(devices);
            }
        }
    }
}

impl Default for LinkedDevicesWindow {
    fn default() -> Self {
        let mut config = WindowConfig::default();
        config.set_styles(&[
            WindowStyle::Titled,
            WindowStyle::Closable,
            WindowStyle::Miniaturizable,
            WindowStyle::Resizable,
        ]);
        config.initial_dimensions = Rect::new(0.0, 0.0, 420.0, 400.0);
        Self(Some(Window::with(config, LinkedDevicesDelegate::default())))
    }
}
