use super::footer::SidebarFooter;
use super::people_list::PeopleList;
use super::rail::SpacesRail;
use super::rooms_list::RoomsList;
use super::{Sidebar, SidebarEvent};
use crate::theme::onedark::OneDarkThemeExt;
use gpui::*;
use gpui_component::scroll::ScrollableElement;

impl Sidebar {
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

    fn render_content(&self) -> impl IntoElement {
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

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rail = SpacesRail::new(self.room_list_model.clone());
        let header = self.render_header(cx);
        let content = self.render_content();
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
