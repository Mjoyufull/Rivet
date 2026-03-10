use crate::components::remote_image::avatar_fallback_label;
use crate::models::appearance::{avatar_radius_for, element_radius_small};
use crate::rooms::build_room_sections;
use crate::rooms::{RailSelection, RoomInfo, RoomListModel};
use crate::theme::onedark::OneDarkTheme;
use crate::theme::onedark::OneDarkThemeExt;
use gpui::*;
use gpui_component::{Icon, IconName, Sizable, StyledExt};

pub struct RoomsList {
    model: Option<Entity<RoomListModel>>,
}

impl RoomsList {
    pub fn new(model: Option<Entity<RoomListModel>>) -> Self {
        Self { model }
    }

    fn render_room_item(
        room: RoomInfo,
        selected_room_id: &Option<String>,
        model_entity: &Entity<RoomListModel>,
        theme: OneDarkTheme,
        avatar_radius: Pixels,
        item_radius: Pixels,
    ) -> AnyElement {
        let active = selected_room_id.as_ref() == Some(&room.id);
        let room_id = room.id.clone();
        let room_name = room.name.clone();
        let model_entity_inner = model_entity.clone();

        let has_unread = room.unread_count > 0 || room.highlight_count > 0;
        let text_color = if active || has_unread {
            theme.text
        } else {
            theme.text_muted
        };

        let avatar = if let Some(url) = &room.avatar_url {
            let fallback = avatar_fallback_label(&room_name, &room.id);
            div()
                .size_10()
                .flex_shrink_0()
                .corner_radii(Corners::all(avatar_radius))
                .overflow_hidden()
                .child(
                    crate::components::remote_image::RemoteImage::new(url.clone())
                        .size(px(40.0))
                        .avatar()
                        .fallback_text(fallback),
                )
                .into_any_element()
        } else {
            div()
                .size_10()
                .flex_shrink_0()
                .bg(if active {
                    theme.accent
                } else {
                    theme.sidebar_item_active
                })
                .corner_radii(Corners::all(avatar_radius))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(if active {
                            theme.sidebar_background
                        } else {
                            theme.text
                        })
                        .font_weight(FontWeight::BOLD)
                        .text_sm()
                        .child(
                            room_name
                                .chars()
                                .find(|c| c.is_alphanumeric())
                                .unwrap_or('#')
                                .to_string()
                                .to_uppercase(),
                        ),
                )
                .into_any_element()
        };

        div()
            .id(ElementId::from(SharedString::from(room_id.clone())))
            .group("room-item")
            .px_2()
            .py_1()
            .corner_radii(Corners::all(item_radius))
            .flex()
            .items_center()
            .gap_3()
            .bg(if active {
                theme.sidebar_item_active
            } else {
                hsla(0.0, 0.0, 0.0, 0.0)
            })
            .hover(move |s| {
                if !active {
                    s.bg(theme.sidebar_item_hover)
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
                    .min_w_0()
                    .text_sm()
                    .font_weight(if has_unread {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(text_color)
                    .truncate()
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
                        theme.error
                    } else {
                        theme.sidebar_item_active
                    })
                    .text_color(theme.text)
                    .text_xs()
                    .child(count.to_string())
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .into_any_element()
    }
}

impl IntoElement for RoomsList {
    type Element = Component<Self>;

    fn into_element(self) -> Self::Element {
        Component::new(self)
    }
}

impl RenderOnce for RoomsList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let avatar_radius = avatar_radius_for(px(40.0), cx);
        let item_radius = element_radius_small(cx);

        let rooms_content = if let Some(model_entity) = &self.model {
            let model_read = model_entity.read(cx);
            if model_read.is_loading {
                div()
                    .px_4()
                    .py_2()
                    .text_sm()
                    .text_color(theme.text_muted)
                    .child("Loading...")
                    .into_any_element()
            } else {
                let selected_room_id = model_read.selected_room_id.clone();
                let rooms_to_render: Vec<_> = model_read.rooms.clone();
                let all_rooms: Vec<_> = model_read.all_rooms.iter().cloned().collect();
                let model_entity_clone = model_entity.clone();

                let mut content_items: Vec<AnyElement> = Vec::new();

                if let RailSelection::Space(selected_space_id) = &model_read.rail_selection {
                    if let Some(space) = all_rooms.iter().find(|room| room.id == *selected_space_id)
                    {
                        let model_entity_inner = model_entity.clone();
                        let is_active = selected_room_id.is_none();
                        let icon_color = if is_active {
                            theme.text
                        } else {
                            theme.text_muted
                        };
                        let title = space.name.clone();

                        content_items.push(
                            div()
                                .px_2()
                                .py_1()
                                .corner_radii(Corners::all(item_radius))
                                .flex()
                                .items_center()
                                .gap_3()
                                .bg(if is_active {
                                    theme.sidebar_item_active
                                } else {
                                    gpui::transparent_black()
                                })
                                .cursor_pointer()
                                .hover(move |style| {
                                    if !is_active {
                                        style.bg(theme.sidebar_item_hover)
                                    } else {
                                        style
                                    }
                                })
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    cx.stop_propagation();
                                    model_entity_inner.update(cx, |model, cx| {
                                        model.clear_selected_room(cx);
                                    });
                                })
                                .child(
                                    Icon::new(IconName::FolderOpen)
                                        .with_size(gpui_component::Size::Small)
                                        .text_color(icon_color),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(if is_active {
                                            theme.text
                                        } else {
                                            theme.text_muted
                                        })
                                        .child(title),
                                )
                                .into_any_element(),
                        );
                    }
                }

                for section in
                    build_room_sections(&model_read.rail_selection, &rooms_to_render, &all_rooms)
                {
                    if let Some(heading) = section.heading {
                        content_items.push(
                            div()
                                .mt_3()
                                .px_2()
                                .py_1()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.text_muted)
                                .child(heading)
                                .into_any_element(),
                        );
                    }

                    for room in section.rooms {
                        content_items.push(Self::render_room_item(
                            room,
                            &selected_room_id,
                            &model_entity_clone,
                            *theme,
                            avatar_radius,
                            item_radius,
                        ));
                    }
                }

                if content_items.is_empty() {
                    div()
                        .px_4()
                        .py_2()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child("No rooms")
                        .into_any_element()
                } else {
                    div().children(content_items).into_any_element()
                }
            }
        } else {
            div().child("Disconnected").into_any_element()
        };

        div()
            .mt_4()
            .flex()
            .flex_col()
            .gap_1()
            .px_2()
            .child(rooms_content)
    }
}
