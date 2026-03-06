use gpui::*;

/// Known SSO identity providers with branded styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsoProvider {
    Apple,
    Facebook,
    GitHub,
    GitLab,
    Google,
    Twitter,
    Matrix, // Native Matrix SSO
    Unknown,
}

impl SsoProvider {
    /// Parse provider from identity provider ID.
    pub fn from_idp_id(id: &str) -> Self {
        let id_lower = id.to_lowercase();
        if id_lower.contains("apple") {
            Self::Apple
        } else if id_lower.contains("facebook") {
            Self::Facebook
        } else if id_lower.contains("github") {
            Self::GitHub
        } else if id_lower.contains("gitlab") {
            Self::GitLab
        } else if id_lower.contains("google") {
            Self::Google
        } else if id_lower.contains("twitter") {
            Self::Twitter
        } else {
            Self::Unknown
        }
    }

    /// Get the provider icon (Unicode emoji fallback).
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Apple => "🍎",
            Self::Facebook => "📘",
            Self::GitHub => "🐙",
            Self::GitLab => "🦊",
            Self::Google => "🔍",
            Self::Twitter => "🐦",
            Self::Matrix => "🔗",
            Self::Unknown => "🔐",
        }
    }

    /// Get the background color for the button.
    pub fn bg_color(&self) -> Rgba {
        match self {
            Self::Apple => rgba(0x000000ff),
            Self::Facebook => rgba(0x1877f2ff),
            Self::GitHub => rgba(0x24292eff),
            Self::GitLab => rgba(0xfc6d26ff),
            Self::Google => rgba(0xea4335ff),
            Self::Twitter => rgba(0x1da1f2ff),
            Self::Matrix => rgba(0x0dbd8bff),
            Self::Unknown => rgba(0x6366f1ff),
        }
    }
}

/// An SSO identity provider button.
pub struct SsoButton {
    provider: SsoProvider,
    name: SharedString,
    idp_id: Option<String>,
    is_hovered: bool,
}

impl SsoButton {
    pub fn new(name: impl Into<SharedString>, idp_id: Option<String>) -> Self {
        let name = name.into();
        let provider = idp_id
            .as_ref()
            .map(|id| SsoProvider::from_idp_id(id))
            .unwrap_or(SsoProvider::Matrix);

        Self {
            provider,
            name,
            idp_id,
            is_hovered: false,
        }
    }

    /// Get the identity provider ID for SSO login.
    pub fn idp_id(&self) -> Option<&str> {
        self.idp_id.as_deref()
    }
}

impl Render for SsoButton {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let bg_color = self.provider.bg_color();
        let bg = if self.is_hovered {
            rgb(0x45475a)
        } else {
            rgb(((bg_color.r * 255.0) as u32) << 16
                | ((bg_color.g * 255.0) as u32) << 8
                | ((bg_color.b * 255.0) as u32))
        };

        div()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .bg(bg)
            .text_color(rgb(0xffffff))
            .rounded_lg()
            .cursor_pointer()
            .min_w_64()
            .child(self.provider.icon())
            .child(format!("Continue with {}", self.name))
    }
}
