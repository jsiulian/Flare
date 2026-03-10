use cacao::view::View;

/// Overlay an NSVisualEffectView with a given material behind a view's contents.
/// Safe to call after other subviews have already been added — inserts below them.
pub fn apply_visual_effect(view: &View, material: i32) {
    use cacao::objc_access::ObjcAccess;
    view.with_backing_obj_mut(|view_obj| unsafe {
        use objc::{class, msg_send, sel, sel_impl};
        let vev: *mut objc::runtime::Object = msg_send![class!(NSVisualEffectView), new];
        let _: () = msg_send![vev, setMaterial: material];
        // NSVisualEffectBlendingModeBehindWindow = 0
        let _: () = msg_send![vev, setBlendingMode: 0i32];
        // NSVisualEffectStateFollowsWindowActiveState = 0
        let _: () = msg_send![vev, setState: 0i32];
        let _: () = msg_send![vev, setTranslatesAutoresizingMaskIntoConstraints: objc::runtime::NO];
        let _: () = msg_send![view_obj, addSubview: vev];
        for (attr1, attr2) in &[(1i32, 1i32), (2i32, 2i32), (3i32, 3i32), (4i32, 4i32)] {
            let constraint: *mut objc::runtime::Object = msg_send![
                class!(NSLayoutConstraint),
                constraintWithItem: vev
                attribute: *attr1
                relatedBy: 0i32
                toItem: view_obj
                attribute: *attr2
                multiplier: 1.0_f64
                constant: 0.0_f64
            ];
            let _: () = msg_send![constraint, setActive: objc::runtime::YES];
        }
    });
}

/// NSVisualEffectMaterial constants
pub const MATERIAL_SIDEBAR: i32 = 7;
