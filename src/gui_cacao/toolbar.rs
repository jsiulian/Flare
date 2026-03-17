use cacao::appkit::toolbar::{ItemIdentifier, Toolbar, ToolbarDelegate, ToolbarItem};
use cacao::appkit::App;
use cacao::button::Button;
use cacao::image::{Image, SFSymbol};
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

use super::app::{AppMessage, FlareApp};
use super::menu_action::make_menu_item;

const MENU_ITEM: &str = "flare.menu";
const COMPOSE_ITEM: &str = "flare.compose";

/// Build the hamburger NSMenu once and return a leaked (immortal) pointer.
fn build_hamburger_menu() -> usize {
    unsafe {
        let menu: *mut Object = msg_send![class!(NSMenu), new];
        let _: () = msg_send![menu, setAutoenablesItems: objc::runtime::NO];

        macro_rules! add_item {
            ($title:expr, $msg:expr) => {{
                let item = make_menu_item($title, move || {
                    App::<FlareApp, AppMessage>::dispatch_main($msg);
                });
                let _: () = msg_send![menu, addItem: item];
                let _: () = msg_send![item, release];
            }};
        }

        add_item!("Settings", AppMessage::OpenPreferences);
        let sep: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
        let _: () = msg_send![menu, addItem: sep];

        add_item!("Synchronize Contacts", AppMessage::SyncContacts);
        add_item!("Submit Captcha", AppMessage::SubmitCaptcha);

        let sep: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
        let _: () = msg_send![menu, addItem: sep];

        add_item!("Clear All Messages", AppMessage::ClearAllMessages);
        add_item!("Clear Conversation Messages", AppMessage::ClearChannelMessages);

        let sep: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
        let _: () = msg_send![menu, addItem: sep];

        add_item!("Linked Devices", AppMessage::OpenLinkedDevices);
        add_item!("Unlink Device", AppMessage::UnlinkDevice);
        add_item!(
            "Unlink and Delete Data",
            AppMessage::UnlinkDeviceAndDelete
        );

        // Leak: the menu lives for the app lifetime
        let _: () = msg_send![menu, retain];
        let _: () = msg_send![menu, release]; // balance the initial +1 from `new`
        menu as usize
    }
}

pub struct FlareToolbarDelegate {
    menu_item: ToolbarItem,
    compose_item: ToolbarItem,
}

impl FlareToolbarDelegate {
    pub fn new() -> Self {
        let menu_ptr = build_hamburger_menu();

        let mut menu_item = ToolbarItem::new(MENU_ITEM);
        menu_item.set_title("Menu");
        let mut menu_btn = Button::new("");
        // Use ellipsis.circle for the hamburger-style menu
        // Load "ellipsis.circle" SF Symbol (macOS 11+); fall back to GearShape if unavailable
        let menu_image = {
            use std::ffi::CString;
            let name_cs = CString::new("ellipsis.circle").unwrap_or_default();
            let ns_name: *mut Object = unsafe {
                let s: *mut Object = msg_send![class!(NSString), alloc];
                msg_send![s, initWithUTF8String: name_cs.as_ptr()]
            };
            let nil: *mut Object = std::ptr::null_mut();
            let ns_img: *mut Object = unsafe {
                msg_send![class!(NSImage), imageWithSystemSymbolName: ns_name accessibilityDescription: nil]
            };
            if ns_img.is_null() {
                Image::symbol(SFSymbol::GearShape, "Menu")
            } else {
                Image::with(ns_img)
            }
        };
        menu_btn.set_image(menu_image);
        menu_btn.set_action(move |sender| unsafe {
            #[repr(C)]
            #[derive(Clone, Copy)]
            struct NSPoint {
                x: f64,
                y: f64,
            }
            unsafe impl objc::Encode for NSPoint {
                fn encode() -> objc::Encoding {
                    unsafe { objc::Encoding::from_str("{CGPoint=dd}") }
                }
            }
            let menu = menu_ptr as *mut Object;
            let view = sender as *mut Object;
            let origin = NSPoint { x: 0.0, y: 0.0 };
            let nil: *mut Object = std::ptr::null_mut();
            let _: () = msg_send![
                menu,
                popUpMenuPositioningItem: nil
                atLocation: origin
                inView: view
            ];
        });
        menu_item.set_button(menu_btn);

        let mut compose_item = ToolbarItem::new(COMPOSE_ITEM);
        compose_item.set_title("New Chat");
        let mut compose_btn = Button::new("");
        compose_btn.set_image(Image::symbol(SFSymbol::SquareAndPencil, "New Chat"));
        compose_btn.set_action(|_| {
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::OpenContactPicker);
        });
        compose_item.set_button(compose_btn);

        Self {
            menu_item,
            compose_item,
        }
    }
}

impl ToolbarDelegate for FlareToolbarDelegate {
    const NAME: &'static str = "FlareToolbar";

    fn allowed_item_identifiers(&self) -> Vec<ItemIdentifier> {
        vec![
            ItemIdentifier::Custom(COMPOSE_ITEM),
            ItemIdentifier::FlexibleSpace,
            ItemIdentifier::Custom(MENU_ITEM),
        ]
    }

    fn default_item_identifiers(&self) -> Vec<ItemIdentifier> {
        vec![
            ItemIdentifier::Custom(COMPOSE_ITEM),
            ItemIdentifier::FlexibleSpace,
            ItemIdentifier::Custom(MENU_ITEM),
        ]
    }

    fn item_for(&self, identifier: &str) -> &ToolbarItem {
        match identifier {
            MENU_ITEM => &self.menu_item,
            _ => &self.compose_item,
        }
    }
}

pub type FlareToolbar = Toolbar<FlareToolbarDelegate>;

pub fn create_toolbar() -> FlareToolbar {
    Toolbar::new("FlareToolbar", FlareToolbarDelegate::new())
}
