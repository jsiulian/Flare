use cacao::appkit::window::{Window, WindowConfig, WindowDelegate, WindowStyle};
use cacao::color::Color;
use cacao::geometry::Rect;
use cacao::layout::{Layout, LayoutConstraint};
use cacao::text::{Font, Label, LineBreakMode};
use cacao::view::View;

use crate::core::channel::CoreChannel;

pub struct ChannelInfoDelegate {
    content: View,
    title_label: Label,
    type_label: Label,
    members_label: Label,
}

impl Default for ChannelInfoDelegate {
    fn default() -> Self {
        Self {
            content: View::new(),
            title_label: Label::new(),
            type_label: Label::new(),
            members_label: Label::new(),
        }
    }
}

impl WindowDelegate for ChannelInfoDelegate {
    const NAME: &'static str = "ChannelInfoWindow";

    fn did_load(&mut self, window: Window) {
        window.set_title("Channel Info");
        window.set_minimum_content_size(360.0, 260.0);
        window.set_titlebar_appears_transparent(true);

        self.title_label.set_font(&Font::bold_system(18.));
        self.type_label.set_font(&Font::system(13.));
        self.type_label.set_text_color(Color::SystemGray);
        self.members_label.set_font(&Font::system(13.));
        self.members_label
            .set_line_break_mode(LineBreakMode::WrapWords);

        self.content.add_subview(&self.title_label);
        self.content.add_subview(&self.type_label);
        self.content.add_subview(&self.members_label);

        LayoutConstraint::activate(&[
            self.title_label
                .top
                .constraint_equal_to(&self.content.top)
                .offset(24.),
            self.title_label
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(24.),
            self.title_label
                .trailing
                .constraint_equal_to(&self.content.trailing)
                .offset(-24.),
            self.type_label
                .top
                .constraint_equal_to(&self.title_label.bottom)
                .offset(8.),
            self.type_label
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(24.),
            self.type_label
                .trailing
                .constraint_equal_to(&self.content.trailing)
                .offset(-24.),
            self.members_label
                .top
                .constraint_equal_to(&self.type_label.bottom)
                .offset(16.),
            self.members_label
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(24.),
            self.members_label
                .trailing
                .constraint_equal_to(&self.content.trailing)
                .offset(-24.),
        ]);

        window.set_content_view(&self.content);
    }
}

pub struct ChannelInfoWindow(pub Option<Window<ChannelInfoDelegate>>);

impl ChannelInfoWindow {
    pub fn show_for_channel(&self, channel: &CoreChannel) {
        if let Some(ref w) = self.0 {
            super::center_window(w);
            let d = w.delegate.as_ref().unwrap();
            d.title_label.set_text(&channel.title);
            let kind = if channel.is_group {
                "Group conversation"
            } else {
                "Direct message"
            };
            d.type_label.set_text(kind);
            if channel.is_group && !channel.members.is_empty() {
                d.members_label
                    .set_text(&format!("Members: {}", channel.members.join(", ")));
                d.members_label.set_hidden(false);
            } else if channel.is_group {
                d.members_label.set_text("Group conversation");
                d.members_label.set_hidden(false);
            } else {
                d.members_label.set_hidden(true);
            }
            w.show();
        }
    }
}

impl Default for ChannelInfoWindow {
    fn default() -> Self {
        let mut config = WindowConfig::default();
        config.set_styles(&[
            WindowStyle::Titled,
            WindowStyle::Closable,
            WindowStyle::Miniaturizable,
        ]);
        config.initial_dimensions = Rect::new(0.0, 0.0, 360.0, 260.0);
        Self(Some(Window::with(config, ChannelInfoDelegate::default())))
    }
}
