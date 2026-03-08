use super::{SettingsEvent, SettingsView};
use crate::theme::onedark::OneDarkThemeExt;
use crate::timeline::ChatStyle;
use gpui::*;
use gpui_component::slider::Slider;

impl SettingsView {
    pub(super) fn render_appearance_settings(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text)
                    .child("Appearance"),
            )
            .child(
                div()
                    .p_4()
                    .rounded_lg()
                    .border(px(1.0))
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child("Avatar Radius"),
                    )
                    .child(div().text_color(theme.text_muted).child(
                        "Adjust avatar corner radius from square (0) to full circle (22px).",
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().text_xs().text_color(theme.text_muted).child("Square"))
                            .child(
                                Slider::new(&self.avatar_radius_slider)
                                    .horizontal()
                                    .flex_1(),
                            )
                            .child(div().text_xs().text_color(theme.text_muted).child("Circle")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child(format!("Current: {:.0}px", f32::from(self.avatar_radius))),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(self.render_style_button(ChatStyle::Modern, "Modern", cx))
                            .child(self.render_style_button(ChatStyle::Bubble, "Bubble", cx)),
                    ),
            )
    }

    fn render_style_button(
        &self,
        style: ChatStyle,
        label: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let is_active = self.chat_style == style;
        let view = cx.entity().clone();
        let label_owned = label.to_string();

        div()
            .px_4()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .border(px(1.0))
            .border_color(if is_active {
                theme.accent
            } else {
                theme.border
            })
            .bg(if is_active {
                theme.accent.opacity(0.1)
            } else {
                gpui::transparent_black()
            })
            .text_color(if is_active { theme.accent } else { theme.text })
            .child(label_owned)
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                view.update(cx, |this, cx| {
                    this.chat_style = style;
                    cx.emit(SettingsEvent::SetChatStyle(style));
                    cx.notify();
                });
            })
    }
}
