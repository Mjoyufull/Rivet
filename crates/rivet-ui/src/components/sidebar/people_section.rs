use gpui::*;
use gpui_component::sidebar::SidebarGroup;

pub struct PeopleSection;

impl Render for PeopleSection {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        SidebarGroup::new("people-group")
            .header(div().child("PEOPLE"))
            .child(div().child("No DMs yet"))
    }
}
