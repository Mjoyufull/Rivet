use super::AppView;
use crate::components::remote_image::{RemoteImage, avatar_fallback_label};
use crate::models::appearance::avatar_radius_for;
use crate::models::ui_preferences;
use crate::rooms::{RailSelection, RoomInfo, RoomListModel, build_room_sections};
use crate::security::verification::SasVerificationPage;
use crate::sidebar::SpacesRail;
use crate::theme::onedark::OneDarkThemeExt;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::scroll::ScrollableElement;

const SIDEBAR_MIN_WIDTH: f32 = 216.0;
const SIDEBAR_MAX_WIDTH: f32 = 420.0;
const SIDEBAR_RESIZE_HANDLE_WIDTH: f32 = 8.0;

#[derive(Debug, Clone)]
struct DraggedSidebarHandle;

fn render_space_avatar(
    space: &RoomInfo,
    size: Pixels,
    radius: Pixels,
    theme: crate::theme::onedark::OneDarkTheme,
) -> AnyElement {
    if let Some(url) = &space.avatar_url {
        let fallback = avatar_fallback_label(&space.name, &space.id);
        RemoteImage::new(url.clone())
            .size(size)
            .avatar()
            .fallback_text(fallback)
            .into_any_element()
    } else {
        div()
            .size(size)
            .corner_radii(Corners::all(radius))
            .bg(theme.sidebar_item_active)
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_color(theme.accent)
                    .font_weight(FontWeight::BOLD)
                    .child(
                        space
                            .name
                            .chars()
                            .find(|c| c.is_alphanumeric())
                            .unwrap_or('#')
                            .to_string()
                            .to_uppercase(),
                    ),
            )
            .into_any_element()
    }
}

fn render_space_home_content(room_list_model: &Entity<RoomListModel>, cx: &mut App) -> AnyElement {
    let theme = *cx.onedark_theme();
    let hero_avatar_radius = avatar_radius_for(px(88.0), cx);
    let room_avatar_radius = avatar_radius_for(px(40.0), cx);
    let model = room_list_model.read(cx);
    let RailSelection::Space(space_id) = &model.rail_selection else {
        return div().into_any_element();
    };
    let Some(space) = model
        .all_rooms
        .iter()
        .find(|room| room.id == *space_id)
        .cloned()
    else {
        return div().into_any_element();
    };

    let all_rooms: Vec<_> = model.all_rooms.iter().cloned().collect();
    let sections = build_room_sections(&model.rail_selection, &model.rooms, &all_rooms);
    let visible_room_count = sections
        .iter()
        .map(|section| section.rooms.len())
        .sum::<usize>();
    let room_list_model = room_list_model.clone();
    let space_name = space.name.clone();

    div()
        .size_full()
        .bg(theme.background)
        .overflow_y_scrollbar()
        .child(
            div()
                .max_w(px(980.0))
                .mx_auto()
                .px_8()
                .py_8()
                .flex()
                .flex_col()
                .gap_6()
                .child(
                    div().flex().items_start().justify_between().gap_6().child(
                        div()
                            .flex()
                            .items_start()
                            .gap_5()
                            .child(render_space_avatar(
                                &space,
                                px(88.0),
                                hero_avatar_radius,
                                theme,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.text_muted)
                                            .child("Welcome to"),
                                    )
                                    .child(
                                        div()
                                            .text_3xl()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme.text)
                                            .child(space_name.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .text_sm()
                                            .text_color(theme.text_muted)
                                            .child("Space")
                                            .child(format!("{visible_room_count} rooms")),
                                    ),
                            ),
                    ),
                )
                .child(div().h_px().w_full().bg(theme.border.opacity(0.7)))
                .children(sections.into_iter().map(|section| {
                    let room_list_model = room_list_model.clone();
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .when_some(section.heading, |this, heading| {
                            this.child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text_muted)
                                    .child(heading),
                            )
                        })
                        .children(section.rooms.into_iter().map(move |room| {
                            let room_id = room.id.clone();
                            let room_list_model = room_list_model.clone();
                            div()
                                .px_4()
                                .py_3()
                                .corner_radii(Corners::all(px(12.0)))
                                .bg(theme.sidebar_background)
                                .border(px(1.0))
                                .border_color(theme.border)
                                .cursor_pointer()
                                .hover(|style| style.bg(theme.sidebar_item_hover))
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    cx.stop_propagation();
                                    room_list_model.update(cx, |model, cx| {
                                        model.select_room(room_id.clone(), cx);
                                    });
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .child(render_space_avatar(
                                            &room,
                                            px(40.0),
                                            room_avatar_radius,
                                            theme,
                                        ))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .flex()
                                                .flex_col()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(theme.text)
                                                        .truncate()
                                                        .child(room.name.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.text_muted)
                                                        .child(room.id.clone()),
                                                ),
                                        ),
                                )
                        }))
                })),
        )
        .into_any_element()
}

impl AppView {
    fn render_sidebar_resize_handle(
        &self,
        theme: crate::theme::onedark::OneDarkTheme,
    ) -> AnyElement {
        div()
            .relative()
            .h_full()
            .flex_shrink_0()
            .w(px(1.0))
            .bg(theme.border)
            .child(
                div()
                    .id("sidebar-resize-handle")
                    .absolute()
                    .left(px(-(SIDEBAR_RESIZE_HANDLE_WIDTH / 2.0)))
                    .w(px(SIDEBAR_RESIZE_HANDLE_WIDTH))
                    .h_full()
                    .cursor_col_resize()
                    .block_mouse_except_scroll()
                    .on_drag(DraggedSidebarHandle, |_, _, _, cx| cx.new(|_| Empty)),
            )
            .into_any_element()
    }

    fn resize_sidebar(
        &mut self,
        drag_event: &DragMoveEvent<DraggedSidebarHandle>,
        cx: &mut Context<Self>,
    ) {
        let bounds = drag_event.bounds;
        let drag_position = drag_event.event.position;
        let next_width = drag_position.x - bounds.left();
        self.sidebar_width = next_width.clamp(px(SIDEBAR_MIN_WIDTH), px(SIDEBAR_MAX_WIDTH));
        cx.notify();
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.active_timeline_model.is_some() && self.active_chat_view.is_none() {
            let model = self.active_timeline_model.as_ref().unwrap().clone();
            let room_list_model = self.room_list_model.as_ref().unwrap().clone();
            let details_panel_preferences = self.details_panel_preferences.clone();
            let chat_view = cx.new(|cx| {
                crate::components::chat::ChatView::new(
                    room_list_model,
                    model,
                    details_panel_preferences,
                    window,
                    cx,
                )
            });
            self.active_chat_view = Some(chat_view);
        }
        let theme = *cx.onedark_theme();
        let navigation_preferences = ui_preferences::ui_preferences(cx);

        if !self.is_logged_in {
            return div().size_full().child(self.login_view.clone());
        }

        if self.verification_gate_active {
            if let Some(vm) = &self.verification_model {
                return div()
                    .size_full()
                    .child(cx.new(|_| SasVerificationPage::new(vm.clone())));
            }

            return div()
                .size_full()
                .bg(theme.background)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    div()
                        .w_16()
                        .h_16()
                        .rounded_full()
                        .bg(theme.sidebar_item_active)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.accent)
                                .child("R"),
                        ),
                )
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text)
                        .child("Verifying this session..."),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child("Secure your keys before loading the main interface."),
                );
        }

        if !self.is_initial_sync_complete(cx) {
            return div()
                .size_full()
                .bg(theme.background)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    div()
                        .w_16()
                        .h_16()
                        .rounded_full()
                        .bg(theme.sidebar_item_active)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.accent)
                                .child("R"),
                        ),
                )
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text)
                        .child("Syncing your data..."),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child(format!("Status: {}", self.sync_status)),
                );
        }

        let main_content = if let Some(vm) = &self.verification_model {
            cx.new(|_| SasVerificationPage::new(vm.clone()))
                .into_any_element()
        } else if let Some(chat_view) = &self.active_chat_view {
            chat_view.clone().into_any_element()
        } else if self.active_room_id.is_some() {
            div()
                .size_full()
                .bg(theme.background)
                .child(
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.text_muted)
                                .child("Loading messages..."),
                        )
                        .into_any_element(),
                )
                .into_any_element()
        } else if let Some(room_list_model) = &self.room_list_model {
            match room_list_model.read(cx).rail_selection {
                RailSelection::Space(_) => render_space_home_content(room_list_model, cx),
                RailSelection::Home | RailSelection::Rooms => div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .child(
                        div()
                            .w_24()
                            .h_24()
                            .bg(theme.sidebar_item_active)
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                if let Some(url) = self.sidebar.read(cx).avatar_url.clone() {
                                    let fallback = avatar_fallback_label(
                                        &self.sidebar.read(cx).display_name,
                                        &self.sidebar.read(cx).user_id,
                                    );
                                    crate::components::remote_image::RemoteImage::new(url.clone())
                                        .size(px(96.0))
                                        .avatar()
                                        .fallback_text(fallback)
                                        .into_any_element()
                                } else {
                                    div()
                                        .w_24()
                                        .h_24()
                                        .rounded_full()
                                        .bg(theme.accent)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            div()
                                                .text_color(theme.sidebar_background)
                                                .font_weight(FontWeight::BOLD)
                                                .child(
                                                    self.sidebar
                                                        .read(cx)
                                                        .display_name
                                                        .chars()
                                                        .next()
                                                        .unwrap_or('U')
                                                        .to_string()
                                                        .to_uppercase(),
                                                )
                                                .into_any_element(),
                                        )
                                        .into_any_element()
                                },
                            ),
                    )
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_muted)
                            .child("Select a room to start chatting"),
                    )
                    .into_any_element(),
            }
        } else {
            div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    div()
                        .w_24()
                        .h_24()
                        .bg(theme.sidebar_item_active)
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            if let Some(url) = self.sidebar.read(cx).avatar_url.clone() {
                                let fallback = avatar_fallback_label(
                                    &self.sidebar.read(cx).display_name,
                                    &self.sidebar.read(cx).user_id,
                                );
                                crate::components::remote_image::RemoteImage::new(url.clone())
                                    .size(px(96.0))
                                    .avatar()
                                    .fallback_text(fallback)
                                    .into_any_element()
                            } else {
                                div()
                                    .w_24()
                                    .h_24()
                                    .rounded_full()
                                    .bg(theme.accent)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_color(theme.sidebar_background)
                                            .font_weight(FontWeight::BOLD)
                                            .child(
                                                self.sidebar
                                                    .read(cx)
                                                    .display_name
                                                    .chars()
                                                    .next()
                                                    .unwrap_or('U')
                                                    .to_string()
                                                    .to_uppercase(),
                                            )
                                            .into_any_element(),
                                    )
                                    .into_any_element()
                            },
                        ),
                )
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text_muted)
                        .child("Select a room to start chatting"),
                )
                .into_any_element()
        };
        let sidebar_width = self
            .sidebar_width
            .clamp(px(SIDEBAR_MIN_WIDTH), px(SIDEBAR_MAX_WIDTH));

        div()
            .relative()
            .size_full()
            .child(
                div()
                    .size_full()
                    .bg(theme.background)
                    .text_color(theme.text)
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .when(navigation_preferences.show_sidecart, |this| {
                                this.child(SpacesRail::new(self.room_list_model.clone()))
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .on_drag_move::<DraggedSidebarHandle>(cx.listener(
                                        |this, event, _window, cx| {
                                            this.resize_sidebar(event, cx);
                                        },
                                    ))
                                    .child(
                                        div()
                                            .w(sidebar_width)
                                            .h_full()
                                            .flex_shrink_0()
                                            .child(self.sidebar.clone()),
                                    )
                                    .child(self.render_sidebar_resize_handle(theme))
                                    .child(div().flex_1().h_full().min_w_0().child(main_content)),
                            ),
                    ),
            )
            .when(self.is_settings_open, |el| {
                el.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .child(self.settings_view.clone()),
                )
            })
    }
}
