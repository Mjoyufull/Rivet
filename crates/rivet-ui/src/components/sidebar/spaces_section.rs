use gpui::*;
use gpui_component::sidebar::SidebarGroup;

pub struct SpacesSection;

impl Render for SpacesSection {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        SidebarGroup::new("spaces-group")
            .header(div().child("SPACES"))
            .child(div().child("No spaces yet"))
    }
}
