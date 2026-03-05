use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};

static CALLBACK_PTR: &str = "rstTextFieldActionCallbackPtr";

type Callback = Box<dyn Fn() + Send + Sync + 'static>;

extern "C" fn perform(this: &Object, _cmd: Sel, _sender: *const Object) {
    unsafe {
        let ptr: usize = *this.get_ivar(CALLBACK_PTR);
        if ptr != 0 {
            let callback = &*(ptr as *const Callback);
            callback();
        }
    }
}

fn register_action_class() -> *const Class {
    static ONCE: std::sync::Once = std::sync::Once::new();
    static mut CLASS: *const Class = std::ptr::null();

    ONCE.call_once(|| unsafe {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("RSTTextFieldActionHandler", superclass).unwrap();
        decl.add_ivar::<usize>(CALLBACK_PTR);
        decl.add_method(
            sel!(perform:),
            perform as extern "C" fn(&Object, Sel, *const Object),
        );
        CLASS = decl.register();
    });

    unsafe { CLASS }
}

/// Wires up a Rust callback to fire when Return is pressed in an NSTextField.
/// The returned handler must be kept alive for as long as the text field is active.
pub struct TextFieldActionHandler {
    invoker: *mut Object,
    _callback: Box<Callback>,
}

unsafe impl Send for TextFieldActionHandler {}
unsafe impl Sync for TextFieldActionHandler {}

impl TextFieldActionHandler {
    pub fn new<F: Fn() + Send + Sync + 'static>(text_field_obj: *mut Object, callback: F) -> Self {
        let cb: Callback = Box::new(callback);
        let cb_box = Box::new(cb);
        let ptr = Box::into_raw(cb_box);

        let class = register_action_class();
        let invoker = unsafe {
            let invoker: *mut Object = msg_send![class, alloc];
            let invoker: *mut Object = msg_send![invoker, init];
            (*invoker).set_ivar(CALLBACK_PTR, ptr as usize);
            let _: () = msg_send![text_field_obj, setAction: sel!(perform:)];
            let _: () = msg_send![text_field_obj, setTarget: invoker];
            invoker
        };

        TextFieldActionHandler {
            invoker,
            _callback: unsafe { Box::from_raw(ptr) },
        }
    }
}

impl Drop for TextFieldActionHandler {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![self.invoker, release];
        }
    }
}
