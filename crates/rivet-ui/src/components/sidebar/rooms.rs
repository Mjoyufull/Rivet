use crate::models::appearance::avatar_radius_for;
use crate::models::rooms_model::{RoomInfo, RoomListModel};
use crate::theme::onedark::OneDarkTheme;
use crate::theme::onedark::OneDarkThemeExt;
use gpui::*;
use gpui_component::StyledExt;
use std::collections::{HashMap, HashSet, VecDeque};

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
            div()
                .size_10()
                .corner_radii(Corners::all(avatar_radius))
                .overflow_hidden()
                .child(
                    crate::components::remote_image::RemoteImage::new(url.clone())
                        .size(px(40.0))
                        .avatar(),
                )
                .into_any_element()
        } else {
            div()
                .size_10()
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
            .rounded_md()
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

    fn resolve_direct_child_space(
        room: &RoomInfo,
        selected_space_id: &str,
        direct_child_space_ids: &HashSet<String>,
        joined_spaces_by_id: &HashMap<String, RoomInfo>,
    ) -> Option<String> {
        let mut queue: VecDeque<String> = room.parent_spaces.iter().cloned().collect();
        let mut visited = HashSet::new();

        while let Some(space_id) = queue.pop_front() {
            if !visited.insert(space_id.clone()) {
                continue;
            }

            if space_id == selected_space_id {
                continue;
            }

            if direct_child_space_ids.contains(&space_id) {
                return Some(space_id);
            }

            if let Some(space) = joined_spaces_by_id.get(&space_id) {
                for parent in &space.parent_spaces {
                    queue.push_back(parent.clone());
                }
            }
        }

        None
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
                let selected_space_id = model_read.selected_space_id.clone();
                let rooms_to_render: Vec<_> = model_read.rooms.clone();
                let all_rooms: Vec<_> = model_read.all_rooms.iter().cloned().collect();
                let model_entity_clone = model_entity.clone();

                let mut content_items: Vec<AnyElement> = Vec::new();

                if let Some(selected_space_id) = selected_space_id {
                    let joined_spaces_by_id: HashMap<String, RoomInfo> = all_rooms
                        .into_iter()
                        .filter(|room| room.is_space && room.is_joined)
                        .map(|room| (room.id.clone(), room))
                        .collect();

                    let mut direct_child_spaces: Vec<RoomInfo> = Vec::new();
                    let mut seen_direct_child_spaces = HashSet::new();

                    if let Some(selected_space) = joined_spaces_by_id.get(&selected_space_id) {
                        for child_id in &selected_space.space_children {
                            if let Some(space) = joined_spaces_by_id.get(child_id)
                                && seen_direct_child_spaces.insert(space.id.clone())
                            {
                                direct_child_spaces.push(space.clone());
                            }
                        }
                    }

                    for space in joined_spaces_by_id.values() {
                        if space
                            .parent_spaces
                            .iter()
                            .any(|parent| parent == &selected_space_id)
                            && seen_direct_child_spaces.insert(space.id.clone())
                        {
                            direct_child_spaces.push(space.clone());
                        }
                    }

                    let direct_child_space_ids: HashSet<String> = direct_child_spaces
                        .iter()
                        .map(|space| space.id.clone())
                        .collect();

                    let mut root_rooms = Vec::new();
                    let mut grouped_rooms: HashMap<String, Vec<RoomInfo>> = HashMap::new();

                    for room in rooms_to_render {
                        if let Some(child_space_id) = Self::resolve_direct_child_space(
                            &room,
                            &selected_space_id,
                            &direct_child_space_ids,
                            &joined_spaces_by_id,
                        ) {
                            grouped_rooms.entry(child_space_id).or_default().push(room);
                        } else if room
                            .parent_spaces
                            .iter()
                            .any(|parent| parent == &selected_space_id)
                        {
                            root_rooms.push(room);
                        } else {
                            root_rooms.push(room);
                        }
                    }

                    for room in root_rooms {
                        content_items.push(Self::render_room_item(
                            room,
                            &selected_room_id,
                            &model_entity_clone,
                            *theme,
                            avatar_radius,
                        ));
                    }

                    for child_space in direct_child_spaces {
                        if let Some(space_rooms) = grouped_rooms.remove(&child_space.id) {
                            if space_rooms.is_empty() {
                                continue;
                            }

                            content_items.push(
                                div()
                                    .mt_3()
                                    .px_2()
                                    .py_1()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text_muted)
                                    .child(child_space.name.to_uppercase())
                                    .into_any_element(),
                            );

                            for room in space_rooms {
                                content_items.push(Self::render_room_item(
                                    room,
                                    &selected_room_id,
                                    &model_entity_clone,
                                    *theme,
                                    avatar_radius,
                                ));
                            }
                        }
                    }
                } else {
                    for room in rooms_to_render {
                        content_items.push(Self::render_room_item(
                            room,
                            &selected_room_id,
                            &model_entity_clone,
                            *theme,
                            avatar_radius,
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
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_muted)
                    .child("ROOMS"),
            )
            .child(rooms_content)
    }
}
