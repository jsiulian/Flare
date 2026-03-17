mod alert;
mod appearance;
mod app;
pub(super) mod notifications;
mod backend;
mod channel_info;
mod channel_list;
mod contact_picker;
mod linked_devices;
mod menu_action;
mod message_view;
mod preferences_window;
mod text_field_action;
mod toolbar;
mod window;

pub use app::run;

use cacao::appkit::window::Window;

pub fn center_window<Delegate: cacao::appkit::window::WindowDelegate>(w: &Window<Delegate>) {
    unsafe {
        use objc::{msg_send, sel, sel_impl};
        let win = &*w.objc as *const _ as *mut objc::runtime::Object;
        let _: () = msg_send![win, center];
    }
}
