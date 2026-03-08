mod appearance;
mod chrome;
mod events;
mod general;
mod overlay;
mod tabs;

use crate::models::appearance::{get_radius, set_radius};
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
    avatar_radius: Pixels,
    avatar_radius_slider: Entity<SliderState>,
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
        let avatar_radius = px(f32::from(get_radius(cx)).clamp(0.0, 22.0));
        let recovery_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Enter recovery key or passphrase...")
        });
        let avatar_radius_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(22.0)
                .step(1.0)
                .default_value(f32::from(avatar_radius))
        });

        cx.subscribe(
            &avatar_radius_slider,
            |this: &mut Self, _, event: &SliderEvent, cx| match event {
                SliderEvent::Change(value) => {
                    let value = match value {
                        SliderValue::Single(v) => *v,
                        SliderValue::Range(_, end) => *end,
                    };
                    this.set_avatar_radius(px(value), cx);
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
            avatar_radius,
            avatar_radius_slider,
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

    pub fn set_avatar_radius(&mut self, radius: Pixels, cx: &mut Context<Self>) {
        let radius = px(f32::from(radius).clamp(0.0, 22.0));
        self.avatar_radius = radius;
        set_radius(radius, cx);
        cx.notify();
    }

    fn set_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }
}
