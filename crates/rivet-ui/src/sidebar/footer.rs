use crate::components::remote_image::{RemoteImage, avatar_fallback_label};
use crate::models::appearance::avatar_radius_for;
use crate::theme::onedark::OneDarkThemeExt;
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::button::ButtonVariants;

#[derive(IntoElement)]
pub struct SidebarFooter {
    display_name: String,
    user_id: String,
    avatar_url: Option<String>,
    pub on_settings_click: Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>,
}

impl SidebarFooter {
    pub fn new(
        display_name: impl Into<String>,
        user_id: impl Into<String>,
        avatar_url: Option<String>,
        on_settings_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            display_name: display_name.into(),
            user_id: user_id.into(),
            avatar_url,
            on_settings_click: Box::new(on_settings_click),
        }
    }
}

impl RenderOnce for SidebarFooter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.onedark_theme();

        div()
            .h_16()
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .bg(theme.sidebar_background)
            .border_t(px(1.0))
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        // Avatar Placeholder or Image
                        div()
                            .size_10()
                            .corner_radii(Corners::all(avatar_radius_for(px(40.0), cx)))
                            .overflow_hidden()
                            .bg(theme.accent)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(if let Some(url) = self.avatar_url {
                                let fallback =
                                    avatar_fallback_label(&self.display_name, &self.user_id);
                                RemoteImage::new(url)
                                    .size(px(40.0))
                                    .avatar()
                                    .low_priority()
                                    .fallback_text(fallback)
                                    .into_any_element()
                            } else {
                                div()
                                    .text_color(theme.sidebar_background)
                                    .font_weight(FontWeight::BOLD)
                                    .child(
                                        self.display_name
                                            .chars()
                                            .next()
                                            .unwrap_or('U')
                                            .to_string()
                                            .to_uppercase(),
                                    )
                                    .into_any_element()
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text)
                                    .child(self.display_name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child(self.user_id),
                            ),
                    ),
            )
            .child(
                div().flex().items_center().child(
                    gpui_component::button::Button::new("settings-button")
                        .icon(gpui_component::IconName::Settings)
                        .ghost()
                        .on_click(self.on_settings_click),
                ),
            )
    }
}
