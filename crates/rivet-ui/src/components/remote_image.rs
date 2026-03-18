use crate::models::appearance::{avatar_radius_for, get_image_radius};
use crate::models::image_cache::ImagePriority;
use gpui::ObjectFit;
use gpui::*;
use gpui_component::StyledExt;
use std::sync::Arc;

#[derive(IntoElement)]
pub struct RemoteImage {
    url: String,
    source: Option<matrix_sdk::ruma::events::room::MediaSource>,
    mimetype: Option<String>,
    size: Option<Pixels>,
    frame_size: Option<Size<Pixels>>,
    is_avatar: bool,
    priority: ImagePriority,
    object_fit: Option<ObjectFit>,
    fallback_text: Option<String>,
}

pub fn avatar_fallback_label(primary: &str, secondary: &str) -> String {
    primary
        .chars()
        .find(|c| c.is_alphanumeric())
        .or_else(|| secondary.chars().find(|c| c.is_alphanumeric()))
        .unwrap_or('?')
        .to_string()
        .to_uppercase()
}

impl RemoteImage {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            source: None,
            mimetype: None,
            size: None,
            frame_size: None,
            is_avatar: false,
            priority: ImagePriority::Normal,
            object_fit: None,
            fallback_text: None,
        }
    }

    pub fn with_source(mut self, source: matrix_sdk::ruma::events::room::MediaSource) -> Self {
        self.source = Some(source);
        self
    }

    pub fn with_mimetype(mut self, mimetype: Option<String>) -> Self {
        self.mimetype = mimetype;
        self
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = Some(size);
        self
    }

    pub fn frame_size(mut self, width: Pixels, height: Pixels) -> Self {
        self.frame_size = Some(size(width, height));
        self
    }

    pub fn object_fit(mut self, fit: ObjectFit) -> Self {
        self.object_fit = Some(fit);
        self
    }

    pub fn fallback_text(mut self, text: impl Into<String>) -> Self {
        self.fallback_text = Some(text.into());
        self
    }

    pub fn high_priority(mut self) -> Self {
        self.priority = ImagePriority::High;
        self
    }

    pub fn low_priority(mut self) -> Self {
        self.priority = ImagePriority::Low;
        self
    }

    /// Mark this image as an avatar for thumbnail optimization
    pub fn avatar(mut self) -> Self {
        self.is_avatar = true;
        // Avatars usually look best with cover fit
        if self.object_fit.is_none() {
            self.object_fit = Some(ObjectFit::Cover);
        }
        self
    }
}

impl RenderOnce for RemoteImage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let requester = window.current_view();
        let image = if self.url.is_empty() && self.source.is_none() {
            None
        } else if let Some(source) = self.source {
            cx.update_global::<crate::models::image_cache::ImageCache, Option<Arc<Image>>>(
                |this, cx| {
                    this.get_media_source(
                        source,
                        self.url.clone(),
                        self.mimetype.clone(),
                        requester,
                        self.priority,
                        cx,
                    )
                },
            )
        } else if self.is_avatar {
            cx.update_global::<crate::models::image_cache::ImageCache, Option<Arc<Image>>>(
                |this, cx| this.get_avatar(self.url.clone(), requester, self.priority, cx),
            )
        } else {
            cx.update_global::<crate::models::image_cache::ImageCache, Option<Arc<Image>>>(
                |this, cx| this.get(self.url.clone(), requester, self.priority, cx),
            )
        };

        let radius = if let Some(size) = self.size {
            avatar_radius_for(size, cx)
        } else if let Some(frame_size) = self.frame_size {
            avatar_radius_for(frame_size.width.min(frame_size.height), cx)
        } else {
            get_image_radius(cx)
        };
        let corner_radii = Corners::all(radius);

        if let Some(image) = image {
            let element = img(ImageSource::Image(image.clone()))
                .object_fit(self.object_fit.unwrap_or(ObjectFit::Cover))
                .corner_radii(corner_radii);

            if let Some(size) = self.size {
                div()
                    .size(size)
                    .flex_shrink_0()
                    .overflow_hidden()
                    .corner_radii(corner_radii)
                    .child(element.size_full())
                    .into_any_element()
            } else if let Some(frame_size) = self.frame_size {
                div()
                    .w(frame_size.width)
                    .h(frame_size.height)
                    .flex_shrink_0()
                    .overflow_hidden()
                    .corner_radii(corner_radii)
                    .child(element.size_full())
                    .into_any_element()
            } else {
                element.into_any_element()
            }
        } else {
            // Loading placeholder
            let placeholder = div()
                .bg(if self.is_avatar {
                    rgb(0x313846)
                } else {
                    rgb(0x282c34)
                })
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(if self.is_avatar {
                            rgb(0x61afef)
                        } else {
                            rgb(0x5c6370)
                        })
                        .font_weight(FontWeight::BOLD)
                        .child(self.fallback_text.unwrap_or_else(|| {
                            if self.is_avatar {
                                "?".into()
                            } else {
                                "...".into()
                            }
                        })),
                );

            if let Some(size) = self.size {
                placeholder
                    .size(size)
                    .flex_shrink_0()
                    .corner_radii(corner_radii)
                    .into_any_element()
            } else if let Some(frame_size) = self.frame_size {
                placeholder
                    .w(frame_size.width)
                    .h(frame_size.height)
                    .flex_shrink_0()
                    .corner_radii(corner_radii)
                    .into_any_element()
            } else {
                placeholder.into_any_element()
            }
        }
    }
}
