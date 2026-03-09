use crate::components::remote_image::avatar_fallback_label;
use crate::models::appearance::{avatar_radius_for, element_radius_small};
use crate::rooms::RoomListModel;
use crate::theme::onedark::OneDarkThemeExt;
use gpui::*;
use gpui_component::StyledExt;

pub struct PeopleList {
    model: Option<Entity<RoomListModel>>,
}

impl PeopleList {
    pub fn new(model: Option<Entity<RoomListModel>>) -> Self {
        Self { model }
    }
}

impl IntoElement for PeopleList {
    type Element = Component<Self>;

    fn into_element(self) -> Self::Element {
        Component::new(self)
    }
}

impl RenderOnce for PeopleList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let avatar_radius = avatar_radius_for(px(40.0), cx);
        let item_radius = element_radius_small(cx);

        let people_content = if let Some(model_entity) = &self.model {
            let model_read = model_entity.read(cx);
            if model_read.is_loading {
                div()
                    .px_4()
                    .py_2()
                    .text_sm()
                    .text_color(theme.text_muted)
                    .child("Loading...")
            } else {
                let selected_id = model_read.selected_room_id.clone();
                let people_to_render: Vec<_> = model_read.people.clone();
                let model_entity_clone = model_entity.clone();
                let theme_clone = theme.clone();

                div().children(people_to_render.into_iter().map(move |room| {
                    let active = selected_id.as_ref() == Some(&room.id);
                    let room_id = room.id.clone();
                    let room_name = room.name.clone();
                    let model_entity_inner = model_entity_clone.clone();
                    let theme_inner = theme_clone;

                    let has_unread = room.unread_count > 0 || room.highlight_count > 0;
                    let text_color = if active {
                        theme_inner.text
                    } else if has_unread {
                        theme_inner.text
                    } else {
                        theme_inner.text_muted
                    };

                    let avatar = if let Some(url) = &room.avatar_url {
                        let fallback = avatar_fallback_label(&room_name, &room.id);
                        div()
                            .size_10()
                            .corner_radii(Corners::all(avatar_radius))
                            .overflow_hidden()
                            .child(
                                crate::components::remote_image::RemoteImage::new(url.clone())
                                    .size(px(40.0))
                                    .avatar()
                                    .fallback_text(fallback),
                            )
                    } else {
                        div()
                            .size_10()
                            .bg(if active {
                                theme_inner.accent
                            } else {
                                theme_inner.sidebar_item_active
                            })
                            .corner_radii(Corners::all(avatar_radius))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_color(if active {
                                        theme_inner.sidebar_background
                                    } else {
                                        theme_inner.text
                                    })
                                    .font_weight(FontWeight::BOLD)
                                    .text_sm()
                                    .child(
                                        room_name
                                            .trim_start_matches('@')
                                            .chars()
                                            .find(|c| c.is_alphanumeric())
                                            .unwrap_or('?')
                                            .to_string()
                                            .to_uppercase(),
                                    ),
                            )
                    };

                    div()
                        .id(ElementId::from(SharedString::from(room_id.clone())))
                        .group("person-item")
                        .px_2()
                        .py_1()
                        .corner_radii(Corners::all(item_radius))
                        .flex()
                        .items_center()
                        .gap_3()
                        .bg(if active {
                            theme_inner.sidebar_item_active
                        } else {
                            hsla(0.0, 0.0, 0.0, 0.0)
                        })
                        .hover(move |s| {
                            if !active {
                                s.bg(theme_inner.sidebar_item_hover)
                            } else {
                                s
                            }
                        })
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                            model_entity_inner.update(cx, |this, cx| {
                                this.select_room(room_id.clone(), cx);
                            });
                        })
                        .child(avatar)
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
                                .child(room_name),
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
                                    theme_inner.error
                                } else {
                                    theme_inner.sidebar_item_active
                                })
                                .text_color(theme_inner.text)
                                .text_xs()
                                .child(count.to_string())
                        } else {
                            div()
                        })
                }))
            }
        } else {
            div().child("Disconnected")
        };

        div()
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
                    .child("DIRECT MESSAGES"),
            )
            .child(people_content)
    }
}
