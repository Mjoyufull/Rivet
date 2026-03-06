use gpui::*;
use gpui_component::ThemeColor;

#[derive(Clone, Copy, Debug)]
pub struct OneDarkTheme {
    pub background: Hsla,
    pub sidebar_background: Hsla,
    pub sidebar_item_hover: Hsla,
    pub sidebar_item_active: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub accent: Hsla,
    pub border: Hsla,
    pub scrollbar: Hsla,
    pub scrollbar_thumb: Hsla,
    pub scrollbar_thumb_hover: Hsla,
    pub window_border: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub error: Hsla,
}

impl OneDarkTheme {
    pub fn dark() -> Self {
        Self {
            background: rgb(0x1e2127).into(),         // Deeper main bg
            sidebar_background: rgb(0x181a1f).into(), // Even darker sidebar
            sidebar_item_hover: rgb(0x282c33).into(),
            sidebar_item_active: rgb(0x2c313a).into(),
            text: rgb(0xabb2bf).into(),       // One Dark Pro main fg
            text_muted: rgb(0x5c6370).into(), // Comment/Muted
            accent: rgb(0x61afef).into(),     // Blue
            border: rgb(0x121417).into(),     // Distinctly darker for rail/borders
            scrollbar: hsla(0.0, 0.0, 0.0, 0.0).into(), // Transparent track
            scrollbar_thumb: hsla(0.0, 0.0, 1.0, 0.1).into(),
            scrollbar_thumb_hover: hsla(0.0, 0.0, 1.0, 0.2).into(),
            window_border: rgb(0x121417).into(),
            success: rgb(0x98c379).into(), // Green
            warning: rgb(0xe5c07b).into(), // Yellow
            error: rgb(0xe06c75).into(),   // Red
        }
    }

    pub fn to_theme_color(&self) -> ThemeColor {
        let mut c = *ThemeColor::dark();
        c.background = self.background;
        c.sidebar = self.sidebar_background;
        c.sidebar_foreground = self.text;
        c.sidebar_border = self.border;
        c.border = self.border;
        c.window_border = self.window_border;

        c.scrollbar = self.scrollbar;
        c.scrollbar_thumb = self.scrollbar_thumb;
        c.scrollbar_thumb_hover = self.scrollbar_thumb_hover;

        c.accent = self.accent;
        c.accent_foreground = self.background; // assuming contrast

        c
    }
}

pub trait OneDarkThemeExt {
    fn onedark_theme(&self) -> &OneDarkTheme;
}

// Blanket implementation for anything that provides AppContext
// This covers App, Context, and other GPUI contexts.
impl<T: AppContext> OneDarkThemeExt for T {
    fn onedark_theme(&self) -> &OneDarkTheme {
        &ONEDARK_THEME
    }
}

lazy_static::lazy_static! {
    pub static ref ONEDARK_THEME: OneDarkTheme = OneDarkTheme::dark();
}
