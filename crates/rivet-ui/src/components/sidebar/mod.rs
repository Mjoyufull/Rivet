mod footer;
mod people;
mod rooms;

use crate::models::appearance::avatar_radius_for;
use crate::models::rooms_model::RoomListModel;
use crate::theme::onedark::OneDarkThemeExt;
use gpui::StatefulInteractiveElement;
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::scroll::ScrollableElement;
use gpui_component::tooltip::Tooltip;
use rivet_core::client::RivetClient;

use self::footer::SidebarFooter;
use self::people::PeopleList;
use self::rooms::RoomsList;

#[derive(Clone)]
pub struct Sidebar {
    pub client: Option<RivetClient>,
    pub room_list_model: Option<Entity<RoomListModel>>,
    pub user_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub sync_status: String,
}

impl Sidebar {
    pub fn new(
        client: Option<RivetClient>,
        room_list_model: Option<Entity<RoomListModel>>,
    ) -> Self {
        let user_id = client
            .as_ref()
            .and_then(|c| c.user_id())
            .unwrap_or_else(|| "@user:matrix.org".to_string());

        let display_name = user_id
            .strip_prefix('@')
            .and_then(|s| s.split(':').next())
            .unwrap_or("User")
            .to_string();

        Self {
            client,
            room_list_model,
            user_id,
            display_name,
            avatar_url: None,
            sync_status: "Idle".to_string(),
        }
    }

    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();

        div()
            .px_4()
            .py_3()
            .border_b(px(1.0))
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text)
                    .child("Rivet"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child(self.sync_status.clone()),
            )
    }

    fn render_spaces_rail(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();

        let is_home_active = self
            .room_list_model
            .as_ref()
            .map(|m| m.read(cx).selected_space_id.is_none())
            .unwrap_or(true);

        let model_entity_home = self.room_list_model.clone();

        div()
            .w_16()
            .h_full()
            .bg(theme.border)
            .flex()
            .flex_col()
            .items_center()
            .py_3()
            .gap_2()
            .child(
                div()
                    .id("space-home")
                    .w_11()
                    .h_11()
                    .bg(if is_home_active {
                        theme.accent
                    } else {
                        theme.sidebar_item_active
                    })
                    .corner_radii(Corners::all(avatar_radius_for(px(44.0), cx)))
                    .overflow_hidden()
                    .border(px(2.0))
                    .border_color(if is_home_active {
                        theme.accent
                    } else {
                        gpui::transparent_black()
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| {
                        s.corner_radii(Corners::all(avatar_radius_for(px(44.0), cx)))
                            .bg(theme.accent)
                            .border_color(theme.accent.opacity(0.8))
                    })
                    .tooltip(|window, cx| Tooltip::new("Home").build(window, cx))
                    .child(
                        div()
                            .text_color(if is_home_active {
                                theme.sidebar_background
                            } else {
                                theme.text
                            })
                            .font_weight(FontWeight::BOLD)
                            .child("R"),
                    )
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        if let Some(m) = &model_entity_home {
                            m.update(cx, |this, cx| {
                                this.select_space(None, cx);
                            });
                        }
                    }),
            )
            .child(div().w_8().h_px().bg(theme.sidebar_item_hover))
            .child(div().flex_1().overflow_y_scrollbar().child(
                div().flex().flex_col().items_center().gap_2().children(
                    if let Some(model_entity) = &self.room_list_model {
                        let model_read = model_entity.read(cx);
                        let selected_id = model_read.selected_space_id.clone();
                        let model_entity_clone = model_entity.clone();

                        model_read
                            .spaces
                            .iter()
                            .map(|space| {
                                let is_active = selected_id.as_ref() == Some(&space.id);
                                let space_id = space.id.clone();
                                let space_name = space.name.clone();
                                let model_entity_inner = model_entity_clone.clone();

                                div()
                                    .id(ElementId::from(SharedString::from(format!(
                                        "space-pill:{}",
                                        space_id
                                    ))))
                                    .w_11()
                                    .h_11()
                                    .bg(if is_active {
                                        theme.accent
                                    } else {
                                        theme.sidebar_item_active
                                    })
                                    .corner_radii(Corners::all(avatar_radius_for(px(44.0), cx)))
                                    .overflow_hidden()
                                    .border(px(2.0))
                                    .border_color(if is_active {
                                        theme.accent
                                    } else {
                                        gpui::transparent_black()
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .hover(|s| {
                                        s.corner_radii(Corners::all(avatar_radius_for(
                                            px(44.0),
                                            cx,
                                        )))
                                        .bg(theme.accent)
                                        .border_color(theme.accent.opacity(0.8))
                                    })
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(space_name.clone()).build(window, cx)
                                    })
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        model_entity_inner.update(cx, |this, cx| {
                                            this.select_space(Some(space_id.clone()), cx);
                                        });
                                    })
                                    .child(if let Some(url) = &space.avatar_url {
                                        crate::components::remote_image::RemoteImage::new(
                                            url.clone(),
                                        )
                                        .size(px(44.0))
                                        .avatar()
                                        .into_any_element()
                                    } else {
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if is_active {
                                                theme.sidebar_background
                                            } else {
                                                theme.text
                                            })
                                            .child(
                                                space
                                                    .name
                                                    .chars()
                                                    .next()
                                                    .unwrap_or('?')
                                                    .to_string(),
                                            )
                                            .into_any_element()
                                    })
                            })
                            .collect::<Vec<_>>()
                    } else {
                        vec![]
                    },
                ),
            ))
    }

    fn render_content(&self, _cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .overflow_hidden()
            .overflow_y_scrollbar()
            .child(
                div()
                    .py_2()
                    .child(PeopleList::new(self.room_list_model.clone()))
                    .child(RoomsList::new(self.room_list_model.clone())),
            )
    }
}

pub enum SidebarEvent {
    OpenSettings,
}

impl EventEmitter<SidebarEvent> for Sidebar {}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rail = self.render_spaces_rail(cx);
        let header = self.render_header(cx);
        let content = self.render_content(cx);
        let theme = cx.onedark_theme();
        let view = cx.entity().clone();

        div().flex().h_full().child(rail).child(
            div()
                .w_64()
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
