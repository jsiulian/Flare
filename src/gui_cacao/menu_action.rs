use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CString;

type Callback = Box<dyn Fn() + Send + Sync + 'static>;

static CALLBACK_PTR: &str = "rstMenuItemCallbackPtr";

extern "C" fn perform(this: &Object, _cmd: Sel, _sender: *const Object) {
    unsafe {
        let ptr: usize = *this.get_ivar(CALLBACK_PTR);
        if ptr != 0 {
            let callback = &*(ptr as *const Callback);
            callback();
        }
    }
}

extern "C" fn validate_menu_item(
    _this: &Object,
    _cmd: Sel,
    _item: *const Object,
) -> objc::runtime::BOOL {
    objc::runtime::YES
}

fn register_menu_handler_class() -> *const Class {
    static ONCE: std::sync::Once = std::sync::Once::new();
    static mut CLASS: *const Class = std::ptr::null();

    ONCE.call_once(|| unsafe {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("RSTMenuItemActionHandler", superclass).unwrap();
        decl.add_ivar::<usize>(CALLBACK_PTR);
        decl.add_method(
            sel!(perform:),
            perform as extern "C" fn(&Object, Sel, *const Object),
        );
        decl.add_method(
            sel!(validateMenuItem:),
            validate_menu_item as extern "C" fn(&Object, Sel, *const Object) -> objc::runtime::BOOL,
        );
        CLASS = decl.register();
    });

    unsafe { CLASS }
}

/// Create a handler object without creating an NSMenuItem.
/// The handler is intentionally leaked (+1) because NSMenuItem/NSControl
/// do NOT retain their target (it is a weak/assign property).
pub unsafe fn create_action_handler(callback: impl Fn() + Send + Sync + 'static) -> *mut Object {
    let cb: Callback = Box::new(callback);
    let cb_ptr = Box::into_raw(Box::new(cb)) as usize;
    let class = register_menu_handler_class();
    let handler: *mut Object = msg_send![class, alloc];
    let handler: *mut Object = msg_send![handler, init];
    unsafe { (*handler).set_ivar(CALLBACK_PTR, cb_ptr) };
    handler
}

pub(super) unsafe fn nsstring(s: &str) -> *mut Object {
    let cs = CString::new(s).unwrap_or_default();
    let obj: *mut Object = msg_send![class!(NSString), alloc];
    msg_send![obj, initWithUTF8String: cs.as_ptr()]
}

/// Create an NSMenuItem whose action calls the given Rust closure.
/// The handler object is retained by the NSMenuItem (target is retained),
/// so the closure lives as long as the menu item does.
/// The callback pointer is intentionally leaked since it is owned by the handler.
pub unsafe fn make_menu_item(
    title: &str,
    callback: impl Fn() + Send + Sync + 'static,
) -> *mut Object {
    let cb: Callback = Box::new(callback);
    let cb_ptr = Box::into_raw(Box::new(cb)) as usize;

    let class = register_menu_handler_class();
    let handler: *mut Object = msg_send![class, alloc];
    let handler: *mut Object = msg_send![handler, init];
    unsafe { (*handler).set_ivar(CALLBACK_PTR, cb_ptr) };

    let ns_title = unsafe { nsstring(title) };
    let empty_key = unsafe { nsstring("") };
    let item: *mut Object = msg_send![class!(NSMenuItem), alloc];
    let item: *mut Object =
        msg_send![item, initWithTitle: ns_title action: sel!(perform:) keyEquivalent: empty_key];
    let _: () = msg_send![item, setTarget: handler];
    // NSMenuItem does NOT retain its target (weak/assign property).
    // Handler is intentionally leaked so it stays alive.

    item
}

/// Attach a context menu to an NSTableView (or any NSView) with Copy Message, Reply, and Delete.
pub fn attach_message_context_menu(
    view_obj: *mut Object,
    copy_cb: impl Fn() + Send + Sync + 'static,
    reply_cb: impl Fn() + Send + Sync + 'static,
    delete_cb: impl Fn() + Send + Sync + 'static,
) {
    let view_ptr = view_obj as usize;

    unsafe {
        let menu: *mut Object = msg_send![class!(NSMenu), new];
        let _: () = msg_send![menu, setAutoenablesItems: objc::runtime::NO];

        let reply_item = make_menu_item("Reply", move || {
            let v = view_ptr as *mut Object;
            let row: isize = msg_send![v, clickedRow];
            super::message_view::LAST_CLICKED_ROW.store(row, std::sync::atomic::Ordering::Relaxed);
            reply_cb();
        });
        let _: () = msg_send![reply_item, setEnabled: objc::runtime::YES];
        let _: () = msg_send![menu, addItem: reply_item];

        let copy_item = make_menu_item("Copy Message", move || {
            let v = view_ptr as *mut Object;
            let row: isize = msg_send![v, clickedRow];
            super::message_view::LAST_CLICKED_ROW.store(row, std::sync::atomic::Ordering::Relaxed);
            copy_cb();
        });
        let _: () = msg_send![copy_item, setEnabled: objc::runtime::YES];
        let _: () = msg_send![menu, addItem: copy_item];

        let sep: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
        let _: () = msg_send![menu, addItem: sep];

        let delete_item = make_menu_item("Delete Message", move || {
            let v = view_ptr as *mut Object;
            let row: isize = msg_send![v, clickedRow];
            super::message_view::LAST_CLICKED_ROW.store(row, std::sync::atomic::Ordering::Relaxed);
            delete_cb();
        });
        let _: () = msg_send![delete_item, setEnabled: objc::runtime::YES];
        let _: () = msg_send![menu, addItem: delete_item];

        let _: () = msg_send![view_obj, setMenu: menu];
        let _: () = msg_send![menu, release];
    }
}
