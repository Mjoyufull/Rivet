use super::{SettingsEvent, SettingsView};
use crate::theme::onedark::OneDarkThemeExt;
use crate::timeline::ChatStyle;
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::slider::Slider;

impl SettingsView {
    pub(super) fn render_appearance_settings(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let card_radius = self.element_radius;
        let inner_card_radius = px((f32::from(card_radius) - 1.0).max(0.0));

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
                    .bg(theme.border)
                    .corner_radii(Corners::all(card_radius))
                    .p(px(1.0))
                    .child(
                        div()
                            .p_4()
                            .bg(theme.background)
                            .corner_radii(Corners::all(inner_card_radius))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child("Image Radius"),
                            )
                            .child(div().text_color(theme.text_muted).child(
                                "Adjust image and avatar corner radius from square (0) to full circle (22px).",
                            ))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(div().text_xs().text_color(theme.text_muted).child("Square"))
                                    .child(
                                        Slider::new(&self.image_radius_slider)
                                            .horizontal()
                                            .flex_1(),
                                    )
                                    .child(div().text_xs().text_color(theme.text_muted).child("Circle")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child(format!("Current: {:.0}px", f32::from(self.image_radius))),
                            ),
                    ),
            )
            .child(
                div()
                    .bg(theme.border)
                    .corner_radii(Corners::all(card_radius))
                    .p(px(1.0))
                    .child(
                        div()
                            .p_4()
                            .bg(theme.background)
                            .corner_radii(Corners::all(inner_card_radius))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child("Element Radius"),
                            )
                            .child(div().text_color(theme.text_muted).child(
                                "Adjust corner radius for panels, cards, message bubbles, and other UI elements.",
                            ))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(div().text_xs().text_color(theme.text_muted).child("Sharp"))
                                    .child(
                                        Slider::new(&self.element_radius_slider)
                                            .horizontal()
                                            .flex_1(),
                                    )
                                    .child(div().text_xs().text_color(theme.text_muted).child("Soft")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child(format!("Current: {:.0}px", f32::from(self.element_radius))),
                            ),
                    )
            )
            .child(
                div()
                    .bg(theme.border)
                    .corner_radii(Corners::all(card_radius))
                    .p(px(1.0))
                    .child(
                        div()
                            .p_4()
                            .bg(theme.background)
                            .corner_radii(Corners::all(inner_card_radius))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child("Timeline Style"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(self.render_style_button(ChatStyle::Modern, "Modern", cx))
                                    .child(self.render_style_button(ChatStyle::Bubble, "Bubble", cx)),
                            ),
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
        let button_radius = px((f32::from(self.element_radius) * 0.75).clamp(0.0, 18.0));
        let button_inner_radius = px((f32::from(button_radius) - 1.0).max(0.0));
        let border_color = if is_active {
            theme.accent
        } else {
            theme.border
        };
        let background = if is_active {
            theme.accent.opacity(0.1)
        } else {
            gpui::transparent_black()
        };
        let text_color = if is_active { theme.accent } else { theme.text };

        div()
            .bg(border_color)
            .corner_radii(Corners::all(button_radius))
            .p(px(1.0))
            .cursor_pointer()
            .text_color(text_color)
            .child(
                div()
                    .px_4()
                    .py_2()
                    .bg(background)
                    .corner_radii(Corners::all(button_inner_radius))
                    .child(label_owned),
            )
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                view.update(cx, |this, cx| {
                    this.chat_style = style;
                    cx.emit(SettingsEvent::SetChatStyle(style));
                    cx.notify();
                });
            })
    }
}
