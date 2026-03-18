use crate::components::remote_image::avatar_fallback_label;
use crate::models::appearance::avatar_radius_for;
use crate::rooms::{RailSelection, RoomListModel};
use crate::theme::onedark::OneDarkThemeExt;
use gpui::StatefulInteractiveElement;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::scroll::ScrollableElement;
use gpui_component::tooltip::Tooltip;

pub(crate) struct SpacesRail {
    model: Option<Entity<RoomListModel>>,
}

impl SpacesRail {
    pub(crate) fn new(model: Option<Entity<RoomListModel>>) -> Self {
        Self { model }
    }
}

impl IntoElement for SpacesRail {
    type Element = Component<Self>;

    fn into_element(self) -> Self::Element {
        Component::new(self)
    }
}

impl RenderOnce for SpacesRail {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.onedark_theme();

        let is_home_active = self
            .model
            .as_ref()
            .map(|m| matches!(m.read(cx).rail_selection, RailSelection::Home))
            .unwrap_or(true);
        let is_rooms_active = self
            .model
            .as_ref()
            .map(|m| matches!(m.read(cx).rail_selection, RailSelection::Rooms))
            .unwrap_or(false);
        let show_other_rooms = self
            .model
            .as_ref()
            .map(|m| m.read(cx).show_other_rooms)
            .unwrap_or(true);

        let model_entity_home = self.model.clone();
        let model_entity_rooms = self.model.clone();

        div()
            .w_16()
            .h_full()
            .flex_shrink_0()
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
                    .child(svg().path("brand/icon.svg").size(rems(2.4)).text_color(
                        if is_home_active {
                            theme.sidebar_background
                        } else {
                            theme.text
                        },
                    ))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        if let Some(m) = &model_entity_home {
                            m.update(cx, |this, cx| {
                                this.select_home(cx);
                            });
                        }
                    }),
            )
            .child(div().w_8().h_px().bg(theme.sidebar_item_hover))
            .when(show_other_rooms, |this: Div| {
                this.child(
                    div()
                        .id("space-rooms")
                        .w_11()
                        .h_11()
                        .bg(if is_rooms_active {
                            theme.accent
                        } else {
                            theme.sidebar_item_active
                        })
                        .corner_radii(Corners::all(avatar_radius_for(px(44.0), cx)))
                        .overflow_hidden()
                        .border(px(2.0))
                        .border_color(if is_rooms_active {
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
                        .tooltip(|window, cx| Tooltip::new("Other Rooms").build(window, cx))
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_rooms_active {
                                    theme.sidebar_background
                                } else {
                                    theme.text
                                })
                                .child("#"),
                        )
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                            if let Some(m) = &model_entity_rooms {
                                m.update(cx, |this, cx| {
                                    this.select_rooms(cx);
                                });
                            }
                        }),
                )
            })
            .child(div().flex_1().overflow_y_scrollbar().child(
                div().flex().flex_col().items_center().gap_2().children(
                    if let Some(model_entity) = &self.model {
                        let model_read = model_entity.read(cx);
                        let selected_id = match &model_read.rail_selection {
                            RailSelection::Space(space_id) => Some(space_id.clone()),
                            RailSelection::Home | RailSelection::Rooms => None,
                        };
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
                                            this.select_space(space_id.clone(), cx);
                                        });
                                    })
                                    .child(if let Some(url) = &space.avatar_url {
                                        let fallback =
                                            avatar_fallback_label(&space.name, &space.id);
                                        crate::components::remote_image::RemoteImage::new(
                                            url.clone(),
                                        )
                                        .size(px(44.0))
                                        .avatar()
                                        .low_priority()
                                        .fallback_text(fallback)
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
}
