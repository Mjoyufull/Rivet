use super::footer::SidebarFooter;
use super::{Sidebar, SidebarEvent};
use crate::components::remote_image::avatar_fallback_label;
use crate::models::appearance::{avatar_radius_for, element_radius_small};
use crate::models::ui_preferences;
use crate::rooms::{RailSelection, RoomInfo, build_room_sections};
use crate::theme::onedark::{OneDarkTheme, OneDarkThemeExt};
use gpui::*;
use gpui_component::scroll::ScrollableElement;
use gpui_component::{Icon, IconName, Sizable, StyledExt};
use std::rc::Rc;

const SIDEBAR_ROW_HEIGHT: Pixels = px(48.0);

#[derive(Clone)]
enum SidebarRow {
    Header(String),
    Person(RoomInfo),
    Room(RoomInfo),
    SpaceHome(RoomInfo),
    Empty(String),
    Loading(String),
}

impl Sidebar {
    fn render_header(&self, cx: &Context<Self>) -> AnyElement {
        let theme = cx.onedark_theme();
        let show_sidecart = ui_preferences::ui_preferences(cx).show_sidecart;
        let room_list_model = self.room_list_model.clone();

        div()
            .px_4()
            .py_3()
            .border_b(px(1.0))
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap_1()
            .child(if show_sidecart {
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text)
                    .child("Rivet")
                    .into_any_element()
            } else {
                div()
                    .h(px(38.0))
                    .w(px(154.0))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .child(
                        svg()
                            .path("brand/horizontal.svg")
                            .w_full()
                            .h_full()
                            .text_color(theme.accent),
                    )
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        if let Some(room_list_model) = &room_list_model {
                            room_list_model.update(cx, |model, cx| {
                                model.select_home(cx);
                            });
                        }
                    })
                    .into_any_element()
            })
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child(self.sync_status.clone()),
            )
            .into_any_element()
    }

    fn build_rows(&self, cx: &App) -> Vec<SidebarRow> {
        let Some(model_entity) = &self.room_list_model else {
            return vec![SidebarRow::Empty("Disconnected".to_string())];
        };

        let model = model_entity.read(cx);
        if model.is_loading {
            return vec![SidebarRow::Loading("Loading...".to_string())];
        }

        let all_rooms: Vec<_> = model.all_rooms.iter().cloned().collect();
        let mut rows = Vec::new();

        if matches!(model.rail_selection, RailSelection::Home) && !model.people.is_empty() {
            rows.push(SidebarRow::Header("DIRECT MESSAGES".to_string()));
            rows.extend(model.people.iter().cloned().map(SidebarRow::Person));
        }

        if let RailSelection::Space(selected_space_id) = &model.rail_selection
            && let Some(space) = all_rooms.iter().find(|room| room.id == *selected_space_id)
        {
            rows.push(SidebarRow::SpaceHome(space.clone()));
        }

        for section in build_room_sections(&model.rail_selection, &model.rooms, &all_rooms) {
            let has_rooms = !section.rooms.is_empty();
            if let Some(heading) = section.heading
                && (has_rooms || rows.is_empty())
            {
                rows.push(SidebarRow::Header(heading));
            }
            rows.extend(section.rooms.into_iter().map(SidebarRow::Room));
        }

        if rows.is_empty() {
            rows.push(SidebarRow::Empty("No rooms".to_string()));
        }

        rows
    }

    fn render_avatar(
        room: &RoomInfo,
        active: bool,
        theme: OneDarkTheme,
        avatar_radius: Pixels,
    ) -> AnyElement {
        if let Some(url) = &room.avatar_url {
            let fallback = avatar_fallback_label(&room.name, &room.id);
            div()
                .size_10()
                .flex_shrink_0()
                .corner_radii(Corners::all(avatar_radius))
                .overflow_hidden()
                .child(
                    crate::components::remote_image::RemoteImage::new(url.clone())
                        .size(px(40.0))
                        .avatar()
                        .low_priority()
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
                            room.name
                                .trim_start_matches('@')
                                .chars()
                                .find(|c| c.is_alphanumeric())
                                .unwrap_or(if room.is_direct { '?' } else { '#' })
                                .to_string()
                                .to_uppercase(),
                        ),
                )
                .into_any_element()
        }
    }

    fn render_room_like_row(
        room: &RoomInfo,
        selected_room_id: &Option<String>,
        model_entity: &Entity<crate::rooms::RoomListModel>,
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

        div()
            .h(SIDEBAR_ROW_HEIGHT)
            .px_2()
            .py_1()
            .child(
                div()
                    .size_full()
                    .id(ElementId::from(SharedString::from(room_id.clone())))
                    .group(if room.is_direct {
                        "person-item"
                    } else {
                        "room-item"
                    })
                    .px_2()
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
                    .child(Self::render_avatar(room, active, theme, avatar_radius))
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
                    }),
            )
            .into_any_element()
    }

    fn render_space_home_row(
        room: &RoomInfo,
        selected_room_id: &Option<String>,
        model_entity: &Entity<crate::rooms::RoomListModel>,
        theme: OneDarkTheme,
        item_radius: Pixels,
    ) -> AnyElement {
        let is_active = selected_room_id.is_none();
        let icon_color = if is_active {
            theme.text
        } else {
            theme.text_muted
        };
        let title = room.name.clone();
        let model_entity_inner = model_entity.clone();

        div()
            .h(SIDEBAR_ROW_HEIGHT)
            .px_2()
            .py_1()
            .child(
                div()
                    .size_full()
                    .px_2()
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
                    ),
            )
            .into_any_element()
    }

    fn render_header_row(label: &str, theme: OneDarkTheme) -> AnyElement {
        div()
            .h(SIDEBAR_ROW_HEIGHT)
            .px_4()
            .flex()
            .items_end()
            .pb_2()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(theme.text_muted)
            .child(label.to_string())
            .into_any_element()
    }

    fn render_empty_row(label: &str, theme: OneDarkTheme) -> AnyElement {
        div()
            .h(SIDEBAR_ROW_HEIGHT)
            .px_4()
            .flex()
            .items_center()
            .text_sm()
            .text_color(theme.text_muted)
            .child(label.to_string())
            .into_any_element()
    }

    fn render_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = *cx.onedark_theme();
        let avatar_radius = avatar_radius_for(px(40.0), cx);
        let item_radius = element_radius_small(cx);
        let rows = Rc::new(self.build_rows(cx));
        let model_entity = self.room_list_model.clone();
        let selected_room_id = model_entity
            .as_ref()
            .and_then(|model| model.read(cx).selected_room_id.clone());
        let scroll_handle = self.scroll_handle.clone();
        let rows_for_list = rows.clone();

        div()
            .flex_1()
            .min_h_0()
            .relative()
            .child(
                uniform_list("sidebar-content", rows.len(), move |range, _, _cx| {
                    let Some(model_entity) = model_entity.as_ref() else {
                        return range
                            .map(|ix| match &rows_for_list[ix] {
                                SidebarRow::Header(label) => {
                                    Sidebar::render_header_row(label, theme)
                                }
                                SidebarRow::Empty(label) | SidebarRow::Loading(label) => {
                                    Sidebar::render_empty_row(label, theme)
                                }
                                SidebarRow::Person(_)
                                | SidebarRow::Room(_)
                                | SidebarRow::SpaceHome(_) => {
                                    Sidebar::render_empty_row("Disconnected", theme)
                                }
                            })
                            .collect();
                    };

                    range
                        .map(|ix| match &rows_for_list[ix] {
                            SidebarRow::Header(label) => Sidebar::render_header_row(label, theme),
                            SidebarRow::Empty(label) | SidebarRow::Loading(label) => {
                                Sidebar::render_empty_row(label, theme)
                            }
                            SidebarRow::Person(room) | SidebarRow::Room(room) => {
                                Sidebar::render_room_like_row(
                                    room,
                                    &selected_room_id,
                                    model_entity,
                                    theme,
                                    avatar_radius,
                                    item_radius,
                                )
                            }
                            SidebarRow::SpaceHome(space) => Sidebar::render_space_home_row(
                                space,
                                &selected_room_id,
                                model_entity,
                                theme,
                                item_radius,
                            ),
                        })
                        .collect()
                })
                .size_full()
                .track_scroll(scroll_handle.clone()),
            )
            .vertical_scrollbar(&scroll_handle)
            .into_any_element()
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = self.render_header(cx);
        let content = self.render_content(cx);
        let theme = cx.onedark_theme();
        let view = cx.entity().clone();

        div().size_full().flex().child(
            div()
                .flex_1()
                .min_w(px(220.0))
                .h_full()
                .flex()
                .flex_col()
                .bg(theme.sidebar_background)
                .border_r_1()
                .border_color(theme.border)
                .child(header)
                .child(content)
                .child(SidebarFooter::new(
                    self.display_name.clone(),
                    self.user_id.clone(),
                    self.avatar_url.clone(),
                    move |_, _, cx| {
                        view.update(cx, |_, cx| {
                            cx.emit(SidebarEvent::OpenSettings);
                        });
                    },
                )),
        )
    }
}
