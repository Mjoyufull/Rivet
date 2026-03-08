use super::{SettingsEvent, SettingsTab, SettingsView};
use crate::theme::onedark::OneDarkThemeExt;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::scroll::ScrollableElement;

impl SettingsView {
    pub(super) fn render_sidebar(
        &self,
        panel_width: Pixels,
        panel_radius: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let view = cx.entity().clone();
        let view_for_logout = view.clone();
        let sidebar_width = px((f32::from(panel_width) * 0.28).clamp(132.0, 256.0));
        let sidebar_corners = Corners {
            top_left: panel_radius,
            top_right: px(0.0),
            bottom_right: px(0.0),
            bottom_left: panel_radius,
        };
        div()
            .w(sidebar_width)
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.sidebar_background)
            .corner_radii(sidebar_corners)
            .border_r(px(1.0))
            .border_color(theme.border)
            .child(
                div().p_4().child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text)
                        .child("Settings"),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .child(self.render_tab_button(SettingsTab::General, "General", cx))
                    .child(self.render_tab_button(SettingsTab::Appearance, "Appearance", cx)),
            )
            .child(
                div()
                    .p_4()
                    .border_t(px(1.0))
                    .border_color(theme.border)
                    .child(
                        Button::new("logout-button")
                            .label("Logout")
                            .icon(gpui_component::IconName::ExternalLink)
                            .danger()
                            .on_click(move |_, _, cx| {
                                view_for_logout.update(cx, |_, cx| cx.emit(SettingsEvent::Logout));
                            }),
                    ),
            )
    }

    fn render_tab_button(
        &self,
        tab: SettingsTab,
        label: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let is_active = self.active_tab == tab;
        let view = cx.entity().clone();
        let hover_bg = theme.sidebar_item_hover;
        let active_bg = theme.sidebar_item_active;
        let accent_color = theme.accent;
        let text_color = theme.text;
        let label_owned = label.to_string();
        let item_radius = px((f32::from(self.element_radius) * 0.75).clamp(0.0, 18.0));

        div()
            .px_3()
            .py_2()
            .corner_radii(Corners::all(item_radius))
            .cursor_pointer()
            .bg(if is_active {
                active_bg
            } else {
                gpui::transparent_black()
            })
            .text_color(if is_active { accent_color } else { text_color })
            .child(label_owned)
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                view.update(cx, |this, cx| this.set_tab(tab, cx));
            })
            .when(!is_active, move |el: Div| {
                el.hover(move |s: StyleRefinement| s.bg(hover_bg))
            })
    }

    pub(super) fn render_content(
        &self,
        panel_width: Pixels,
        panel_radius: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let content_padding = px((f32::from(panel_width) * 0.035).clamp(16.0, 32.0));
        let content_corners = Corners {
            top_left: px(0.0),
            top_right: panel_radius,
            bottom_right: panel_radius,
            bottom_left: px(0.0),
        };

        div()
            .flex_1()
            .h_full()
            .bg(theme.background)
            .corner_radii(content_corners)
            .overflow_hidden()
            .child(
                div()
                    .size_full()
                    .p(content_padding)
                    .overflow_y_scrollbar()
                    .child(match self.active_tab {
                        SettingsTab::General => self.render_general_settings(cx).into_any_element(),
                        SettingsTab::Appearance => {
                            self.render_appearance_settings(cx).into_any_element()
                        }
                    })
                    .child(div().h_4()),
            )
    }
}
