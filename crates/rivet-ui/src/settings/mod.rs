mod appearance;
mod chrome;
mod events;
mod general;
mod overlay;
mod tabs;

use crate::models::appearance::{
    get_element_radius, get_image_radius, set_element_radius, set_image_radius,
};
use crate::timeline::ChatStyle;
use gpui::*;
use gpui_component::input::InputState;
use gpui_component::slider::{SliderEvent, SliderState, SliderValue};

pub use events::SettingsEvent;
use tabs::SettingsTab;

pub struct SettingsView {
    active_tab: SettingsTab,
    focus_handle: FocusHandle,
    show_rooms_in_home: bool,
    session_verified: bool,
    chat_style: ChatStyle,
    image_radius: Pixels,
    image_radius_slider: Entity<SliderState>,
    element_radius: Pixels,
    element_radius_slider: Entity<SliderState>,
    recovery_input: Entity<InputState>,
    recovery_status: Option<Result<(), String>>,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl SettingsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let image_radius = px(f32::from(get_image_radius(cx)).clamp(0.0, 22.0));
        let element_radius = px(f32::from(get_element_radius(cx)).clamp(0.0, 24.0));
        let recovery_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Enter recovery key or passphrase...")
        });
        let image_radius_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(22.0)
                .step(1.0)
                .default_value(f32::from(image_radius))
        });
        let element_radius_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(24.0)
                .step(1.0)
                .default_value(f32::from(element_radius))
        });

        cx.subscribe(
            &image_radius_slider,
            |this: &mut Self, _, event: &SliderEvent, cx| match event {
                SliderEvent::Change(value) => {
                    let value = match value {
                        SliderValue::Single(v) => *v,
                        SliderValue::Range(_, end) => *end,
                    };
                    this.set_image_radius(px(value), cx);
                }
            },
        )
        .detach();

        cx.subscribe(
            &element_radius_slider,
            |this: &mut Self, _, event: &SliderEvent, cx| match event {
                SliderEvent::Change(value) => {
                    let value = match value {
                        SliderValue::Single(v) => *v,
                        SliderValue::Range(_, end) => *end,
                    };
                    this.set_element_radius(px(value), cx);
                }
            },
        )
        .detach();

        Self {
            active_tab: SettingsTab::default(),
            focus_handle: cx.focus_handle(),
            show_rooms_in_home: false,
            session_verified: false,
            chat_style: ChatStyle::default(),
            image_radius,
            image_radius_slider,
            element_radius,
            element_radius_slider,
            recovery_input,
            recovery_status: None,
        }
    }

    pub fn set_recovery_status(
        &mut self,
        status: Option<Result<(), String>>,
        cx: &mut Context<Self>,
    ) {
        self.recovery_status = status;
        cx.notify();
    }

    pub fn set_show_rooms_in_home(&mut self, val: bool, cx: &mut Context<Self>) {
        self.show_rooms_in_home = val;
        cx.notify();
    }

    pub fn set_session_verified(&mut self, val: bool, cx: &mut Context<Self>) {
        self.session_verified = val;
        cx.notify();
    }

    pub fn set_image_radius(&mut self, radius: Pixels, cx: &mut Context<Self>) {
        let radius = px(f32::from(radius).clamp(0.0, 22.0));
        self.image_radius = radius;
        set_image_radius(radius, cx);
        cx.notify();
    }

    pub fn set_element_radius(&mut self, radius: Pixels, cx: &mut Context<Self>) {
        let radius = px(f32::from(radius).clamp(0.0, 24.0));
        self.element_radius = radius;
        set_element_radius(radius, cx);
        cx.notify();
    }

    fn set_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }
}
