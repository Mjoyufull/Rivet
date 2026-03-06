use crate::models::appearance::avatar_radius_for;
use crate::models::rooms_model::{RoomInfo, RoomListModel};
use crate::theme::onedark::OneDarkThemeExt;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::StyledExt;

pub struct SpacesList {
    pub model: Option<Entity<RoomListModel>>,
}

impl SpacesList {
    pub fn new(model: Option<Entity<RoomListModel>>) -> Self {
        Self { model }
    }

    fn render_item(&self, room: &RoomInfo, active: bool, cx: &App) -> impl IntoElement {
        let theme = cx.onedark_theme();

        let has_unread = room.unread_count > 0 || room.highlight_count > 0;
        let text_color = if active {
            theme.text
        } else if has_unread {
            theme.text
        } else {
            theme.text_muted
        };

        div()
            .group("space-item")
            .px_2()
            .py_1()
            .rounded_md()
            .flex()
            .items_center()
            .gap_2()
            .bg(if active {
                theme.sidebar_item_active
            } else {
                hsla(0.0, 0.0, 0.0, 0.0)
            })
            .hover(|s| {
                if !active {
                    s.bg(theme.sidebar_item_hover)
                } else {
                    s
                }
            })
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
            })
            .child(if let Some(url) = &room.avatar_url {
                div()
                    .size_10()
                    .corner_radii(Corners::all(avatar_radius_for(px(40.0), cx)))
                    .overflow_hidden()
                    .child(
                        crate::components::remote_image::RemoteImage::new(url.clone())
                            .size(px(40.0))
                            .avatar(),
                    )
            } else {
                div()
                    .size_10()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(if has_unread {
                        theme.accent
                    } else {
                        theme.text_muted
                    })
                    .child("&")
            })
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .font_weight(if has_unread {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(text_color)
                    .child(room.name.clone()),
            )
            .child(if has_unread {
                let count = if room.highlight_count > 0 {
                    room.highlight_count
                } else {
                    room.unread_count
                };

                div()
                    .px_1_5()
                    .rounded_full()
                    .bg(if room.highlight_count > 0 {
                        theme.error
                    } else {
                        theme.sidebar_item_active
                    })
                    .text_color(theme.text)
                    .text_xs()
                    .child(count.to_string())
            } else {
                div()
            })
    }
}

impl RenderOnce for SpacesList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.onedark_theme();

        let spaces_content = if let Some(model) = &self.model {
            let model = model.read(cx);
            if model.is_loading {
                div()
                    .px_4()
                    .py_2()
                    .text_sm()
                    .text_color(theme.text_muted)
                    .child("Loading...")
            } else if model.spaces.is_empty() {
                div()
                    .px_4()
                    .py_2()
                    .text_sm()
                    .text_color(theme.text_muted)
                    .child("No spaces")
            } else {
                div().children(
                    model
                        .spaces
                        .iter()
                        .map(|room| self.render_item(room, false, cx)),
                )
            }
        } else {
            div().child("Disconnected")
        };

        div()
            .mt_4()
            .flex()
            .flex_col()
            .gap_1()
            .px_2()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_muted)
                    .child("SPACES"),
            )
            .child(spaces_content)
    }
}
