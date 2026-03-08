use super::{SettingsEvent, SettingsTab, SettingsView};
use crate::theme::onedark::OneDarkThemeExt;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::scroll::ScrollableElement;

impl SettingsView {
    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let view = cx.entity().clone();
        let view_for_logout = view.clone();

        div()
            .w_64()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.sidebar_background)
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

        div()
            .px_3()
            .py_2()
            .rounded_md()
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

    pub(super) fn render_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();

        div()
            .flex_1()
            .h_full()
            .bg(theme.background)
            .p_8()
            .overflow_y_scrollbar()
            .child(match self.active_tab {
                SettingsTab::General => self.render_general_settings(cx).into_any_element(),
                SettingsTab::Appearance => self.render_appearance_settings(cx).into_any_element(),
            })
    }
}
