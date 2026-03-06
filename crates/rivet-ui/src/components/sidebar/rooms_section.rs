use gpui::*;
use gpui_component::sidebar::SidebarGroup;

pub struct RoomsSection;

impl Render for RoomsSection {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        SidebarGroup::new("rooms-group")
            .header(div().child("ROOMS"))
            .child(div().child("No rooms yet"))
    }
}
