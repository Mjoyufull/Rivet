use crate::models::appearance::avatar_radius_for;
use crate::models::timeline_model::{
    ChatStyle, RenderedBody, RenderedCallEntry, RenderedReplyPreview, RenderedTimelineItem,
    TimelineModel,
};
use crate::theme::onedark::{OneDarkTheme, OneDarkThemeExt};
use gpui::InteractiveElement as _;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::clipboard::Clipboard;
use gpui_component::scroll::Scrollbar;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

pub struct TimelineView {
    model: Entity<TimelineModel>,
    highlighted_event_id: Option<String>,
    expanded_call_groups: HashSet<String>,
}

impl TimelineView {
    pub fn new(model: Entity<TimelineModel>, _cx: &mut Context<Self>) -> Self {
        Self {
            model,
            highlighted_event_id: None,
            expanded_call_groups: HashSet::new(),
        }
    }

    fn toggle_call_group(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.expanded_call_groups.insert(id.to_string()) {
            self.expanded_call_groups.remove(id);
        }
        cx.notify();
    }

    fn highlight_event(&mut self, event_id: String, cx: &mut Context<Self>) {
        self.highlighted_event_id = Some(event_id);
        cx.notify();
    }
}

#[derive(Clone)]
struct ReplyPreviewInteraction {
    target_event_id: String,
    event_indices: Arc<HashMap<String, usize>>,
    list_state: ListState,
    view: Entity<TimelineView>,
}

fn rendered_item_id(item: &RenderedTimelineItem) -> Option<&str> {
    match item {
        RenderedTimelineItem::Message { id, .. }
        | RenderedTimelineItem::Image { id, .. }
        | RenderedTimelineItem::System { id, .. }
        | RenderedTimelineItem::CallGroup { id, .. } => Some(id),
        RenderedTimelineItem::Separator(_) => None,
    }
}

fn avatar_fallback(sender_name: &str, sender_id: &str) -> String {
    sender_name
        .chars()
        .find(|c| c.is_alphanumeric())
        .or_else(|| {
            sender_id
                .trim_start_matches('@')
                .chars()
                .find(|c| c.is_alphanumeric())
        })
        .unwrap_or('?')
        .to_string()
        .to_uppercase()
}

fn render_avatar(
    avatar_url: Option<&String>,
    sender_name: &str,
    sender_id: &str,
    size: Pixels,
    theme: &OneDarkTheme,
    cx: &App,
) -> AnyElement {
    if let Some(url) = avatar_url {
        crate::components::remote_image::RemoteImage::new(url.clone())
            .size(size)
            .avatar()
            .into_any_element()
    } else {
        div()
            .size(size)
            .flex_shrink_0()
            .corner_radii(Corners::all(avatar_radius_for(size, cx)))
            .bg(theme.accent.opacity(0.2))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.accent)
                    .child(avatar_fallback(sender_name, sender_id)),
            )
            .into_any_element()
    }
}

#[derive(Debug)]
enum BodyBlock {
    Paragraph(String),
    Code {
        language: Option<String>,
        code: String,
    },
}

fn html_to_plain_text(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;

    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }

    output
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
}

fn html_code_language(pre_block: &str) -> Option<String> {
    let marker = "language-";
    let start = pre_block.find(marker)?;
    let suffix = &pre_block[start + marker.len()..];
    let end = suffix
        .find(|ch: char| ch == '"' || ch == '\'' || ch.is_whitespace() || ch == '>')
        .unwrap_or(suffix.len());
    let language = suffix[..end].trim();
    (!language.is_empty()).then(|| language.to_string())
}

fn parse_html_body_blocks(html: &str) -> Vec<BodyBlock> {
    let mut blocks = Vec::new();
    let mut remaining = html;

    while let Some(pre_start) = remaining.find("<pre") {
        let before = html_to_plain_text(&remaining[..pre_start])
            .trim()
            .to_string();
        if !before.is_empty() {
            blocks.extend(parse_body_blocks(&before));
        }

        let Some(pre_end_rel) = remaining[pre_start..].find("</pre>") else {
            let fallback = html_to_plain_text(remaining).trim().to_string();
            if !fallback.is_empty() {
                blocks.extend(parse_body_blocks(&fallback));
            }
            return blocks;
        };

        let pre_end = pre_start + pre_end_rel + "</pre>".len();
        let pre_block = &remaining[pre_start..pre_end];
        let language = html_code_language(pre_block);
        let code_text = if let Some(code_start_rel) = pre_block.find("<code") {
            let code_start = code_start_rel
                + pre_block[code_start_rel..]
                    .find('>')
                    .map(|ix| ix + 1)
                    .unwrap_or(0);
            let code_end = pre_block.find("</code>").unwrap_or(pre_block.len());
            html_to_plain_text(&pre_block[code_start..code_end])
        } else {
            html_to_plain_text(pre_block)
        };
        let code_text = code_text.trim_matches('\n').to_string();
        if !code_text.is_empty() {
            blocks.push(BodyBlock::Code {
                language,
                code: code_text,
            });
        }

        remaining = &remaining[pre_end..];
    }

    let tail = html_to_plain_text(remaining).trim().to_string();
    if !tail.is_empty() {
        blocks.extend(parse_body_blocks(&tail));
    }

    if blocks.is_empty() {
        blocks.push(BodyBlock::Paragraph(html_to_plain_text(html)));
    }

    blocks
}

fn parse_body_blocks(text: &str) -> Vec<BodyBlock> {
    let mut blocks = Vec::new();
    let mut paragraph_lines = Vec::new();
    let mut code_lines = Vec::new();
    let mut code_language: Option<String> = None;
    let mut in_code_block = false;

    let flush_paragraph = |blocks: &mut Vec<BodyBlock>, paragraph_lines: &mut Vec<String>| {
        if !paragraph_lines.is_empty() {
            blocks.push(BodyBlock::Paragraph(paragraph_lines.join("\n")));
            paragraph_lines.clear();
        }
    };

    let flush_code = |blocks: &mut Vec<BodyBlock>,
                      code_lines: &mut Vec<String>,
                      code_language: &mut Option<String>| {
        if !code_lines.is_empty() {
            blocks.push(BodyBlock::Code {
                language: code_language.take(),
                code: code_lines.join("\n"),
            });
            code_lines.clear();
        }
    };

    for line in text.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("```") {
            if in_code_block {
                flush_code(&mut blocks, &mut code_lines, &mut code_language);
                in_code_block = false;
            } else {
                flush_paragraph(&mut blocks, &mut paragraph_lines);
                let language = rest.trim();
                code_language = (!language.is_empty()).then(|| language.to_string());
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_lines.push(line.to_string());
            continue;
        }

        if line.trim().is_empty() {
            flush_paragraph(&mut blocks, &mut paragraph_lines);
        } else {
            paragraph_lines.push(line.to_string());
        }
    }

    if in_code_block {
        flush_code(&mut blocks, &mut code_lines, &mut code_language);
    } else {
        flush_paragraph(&mut blocks, &mut paragraph_lines);
    }

    if blocks.is_empty() {
        blocks.push(BodyBlock::Paragraph(text.to_string()));
    }

    blocks
}

fn render_paragraph(text: &str, is_notice: bool, theme: &OneDarkTheme) -> AnyElement {
    let color = if is_notice {
        theme.text_muted
    } else {
        theme.text
    };
    let lines = text
        .lines()
        .map(|line| {
            div()
                .text_sm()
                .text_color(color)
                .whitespace_normal()
                .child(line.to_string())
                .into_any_element()
        })
        .collect::<Vec<_>>();

    div().flex_col().gap_1().children(lines).into_any_element()
}

fn code_keyword_style(color: Hsla, weight: Option<FontWeight>) -> HighlightStyle {
    HighlightStyle {
        color: Some(color),
        font_weight: weight,
        ..Default::default()
    }
}

fn code_keywords(language: Option<&str>) -> &'static [&'static str] {
    match language.unwrap_or("").to_ascii_lowercase().as_str() {
        "rust" | "rs" => &[
            "as", "async", "await", "break", "const", "crate", "else", "enum", "false", "fn",
            "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
            "return", "self", "Self", "static", "struct", "trait", "true", "type", "use", "where",
            "while",
        ],
        "javascript" | "js" | "typescript" | "ts" | "tsx" | "jsx" => &[
            "async",
            "await",
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "default",
            "else",
            "export",
            "extends",
            "false",
            "finally",
            "for",
            "function",
            "if",
            "import",
            "in",
            "let",
            "new",
            "null",
            "return",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "typeof",
            "undefined",
            "var",
            "while",
            "yield",
        ],
        "bash" | "sh" | "shell" | "zsh" => &[
            "case", "do", "done", "elif", "else", "esac", "export", "fi", "for", "function", "if",
            "in", "local", "then", "unset", "until", "while",
        ],
        "json" => &["false", "null", "true"],
        "yaml" | "yml" => &["false", "null", "true"],
        _ => &[],
    }
}

fn comment_marker(language: Option<&str>) -> Option<&'static str> {
    match language.unwrap_or("").to_ascii_lowercase().as_str() {
        "rust" | "rs" | "javascript" | "js" | "typescript" | "ts" | "tsx" | "jsx" => Some("//"),
        "bash" | "sh" | "shell" | "zsh" | "yaml" | "yml" | "python" | "py" | "toml" => Some("#"),
        _ => None,
    }
}

fn string_ranges(line: &str) -> Vec<std::ops::Range<usize>> {
    let bytes = line.as_bytes();
    let mut ix = 0;
    let mut ranges = Vec::new();

    while ix < bytes.len() {
        let quote = bytes[ix];
        if quote != b'\'' && quote != b'"' {
            ix += 1;
            continue;
        }

        let start = ix;
        ix += 1;
        while ix < bytes.len() {
            if bytes[ix] == b'\\' {
                ix += 2;
                continue;
            }
            if bytes[ix] == quote {
                ix += 1;
                break;
            }
            ix += 1;
        }
        ranges.push(start..ix.min(bytes.len()));
    }

    ranges
}

fn overlaps(existing: &[std::ops::Range<usize>], start: usize, end: usize) -> bool {
    existing
        .iter()
        .any(|range| start < range.end && end > range.start)
}

fn syntax_highlights(
    line: &str,
    language: Option<&str>,
    theme: &OneDarkTheme,
) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    let mut highlights = Vec::new();
    let mut reserved_ranges = Vec::new();

    for range in string_ranges(line) {
        reserved_ranges.push(range.clone());
        highlights.push((range, code_keyword_style(theme.success, None)));
    }

    if let Some(marker) = comment_marker(language).and_then(|marker| line.find(marker)) {
        if !overlaps(&reserved_ranges, marker, line.len()) {
            highlights.push((
                marker..line.len(),
                code_keyword_style(theme.text_muted, Some(FontWeight::MEDIUM)),
            ));
            reserved_ranges.push(marker..line.len());
        }
    }

    let bytes = line.as_bytes();
    let mut ix = 0;
    while ix < bytes.len() {
        if bytes[ix].is_ascii_digit() {
            let start = ix;
            ix += 1;
            while ix < bytes.len() && (bytes[ix].is_ascii_digit() || bytes[ix] == b'.') {
                ix += 1;
            }
            if !overlaps(&reserved_ranges, start, ix) {
                highlights.push((start..ix, code_keyword_style(theme.warning, None)));
            }
            continue;
        }
        ix += 1;
    }

    let keywords = code_keywords(language);
    if keywords.is_empty() {
        return highlights;
    }

    let mut word_start = None;
    for (ix, ch) in line.char_indices() {
        let is_word = ch.is_ascii_alphanumeric() || ch == '_';
        match (word_start, is_word) {
            (None, true) => word_start = Some(ix),
            (Some(start), false) => {
                let end = ix;
                let word = &line[start..end];
                if keywords.contains(&word) && !overlaps(&reserved_ranges, start, end) {
                    highlights.push((
                        start..end,
                        code_keyword_style(theme.accent, Some(FontWeight::BOLD)),
                    ));
                }
                word_start = None;
            }
            _ => {}
        }
    }
    if let Some(start) = word_start {
        let end = line.len();
        let word = &line[start..end];
        if keywords.contains(&word) && !overlaps(&reserved_ranges, start, end) {
            highlights.push((
                start..end,
                code_keyword_style(theme.accent, Some(FontWeight::BOLD)),
            ));
        }
    }

    highlights
}

fn render_code_block(
    item_id: &str,
    language: Option<&str>,
    code: &str,
    theme: &OneDarkTheme,
) -> AnyElement {
    let header_label = language
        .filter(|language| !language.is_empty())
        .unwrap_or("Code")
        .to_string();
    let clipboard_id =
        ElementId::Name(SharedString::from(format!("{item_id}-copy-{}", code.len())));
    let code_lines = if code.is_empty() {
        vec![String::new()]
    } else {
        code.lines().map(ToString::to_string).collect::<Vec<_>>()
    };

    div()
        .rounded_lg()
        .overflow_hidden()
        .border_1()
        .border_color(theme.border)
        .child(
            div()
                .h_8()
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .bg(theme.sidebar_background.opacity(0.7))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_muted)
                        .child(header_label),
                )
                .child(
                    Clipboard::new(clipboard_id)
                        .value(code.to_string())
                        .into_any_element(),
                ),
        )
        .child(
            div()
                .bg(theme.sidebar_background.opacity(0.95))
                .px_3()
                .py_3()
                .flex_col()
                .gap_1()
                .children(code_lines.into_iter().map(|line| {
                    let highlights = syntax_highlights(&line, language, theme);
                    div()
                        .text_sm()
                        .text_color(theme.text)
                        .font_family("monospace")
                        .whitespace_normal()
                        .child(if highlights.is_empty() {
                            line.into_any_element()
                        } else {
                            StyledText::new(line)
                                .with_highlights(highlights)
                                .into_any_element()
                        })
                        .into_any_element()
                })),
        )
        .into_any_element()
}

fn render_rich_body(
    item_id: &str,
    blocks: Vec<BodyBlock>,
    is_notice: bool,
    theme: &OneDarkTheme,
) -> AnyElement {
    let children = blocks
        .into_iter()
        .enumerate()
        .map(|(ix, block)| match block {
            BodyBlock::Paragraph(text) => render_paragraph(&text, is_notice, theme),
            BodyBlock::Code { language, code } => render_code_block(
                &format!("{item_id}-{ix}"),
                language.as_deref(),
                &code,
                theme,
            ),
        })
        .collect::<Vec<_>>();

    div()
        .flex_col()
        .gap_2()
        .children(children)
        .into_any_element()
}

fn render_body(
    item_id: &str,
    body: &RenderedBody,
    is_notice: bool,
    theme: &OneDarkTheme,
) -> AnyElement {
    match body {
        RenderedBody::Plain(text) => div()
            .text_sm()
            .text_color(if is_notice {
                theme.text_muted
            } else {
                theme.text
            })
            .child(text.clone())
            .into_any_element(),
        RenderedBody::Markdown(text) => {
            render_rich_body(item_id, parse_body_blocks(text), is_notice, theme)
        }
        RenderedBody::Html(text) => {
            render_rich_body(item_id, parse_html_body_blocks(text), is_notice, theme)
        }
    }
}

fn render_reply_preview(
    reply: &RenderedReplyPreview,
    interaction: Option<ReplyPreviewInteraction>,
    theme: &OneDarkTheme,
) -> AnyElement {
    let preview = div()
        .rounded_md()
        .border_l_2()
        .border_color(theme.accent.opacity(0.8))
        .bg(theme.sidebar_background.opacity(0.45))
        .px_3()
        .py_2()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.accent)
                .child(reply.sender_name.clone()),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .line_clamp(1)
                .child(reply.body.clone()),
        );

    if let Some(interaction) = interaction {
        let target_event_id = interaction.target_event_id.clone();
        let event_indices = interaction.event_indices.clone();
        let list_state = interaction.list_state.clone();
        let view = interaction.view.clone();

        preview
            .cursor_pointer()
            .hover(|this| this.bg(theme.sidebar_background.opacity(0.7)))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                if let Some(index) = event_indices.get(&target_event_id).copied() {
                    list_state.scroll_to_reveal_item(index);
                    let highlight_id = target_event_id.clone();
                    let _ = view.update(cx, |this, cx| {
                        this.highlight_event(highlight_id.clone(), cx);
                    });

                    let view = view.clone();
                    let clear_id = target_event_id.clone();
                    cx.spawn(async move |cx| {
                        cx.background_executor().timer(Duration::from_secs(2)).await;
                        let _ = view.update(cx, |this, cx| {
                            if this.highlighted_event_id.as_deref() == Some(clear_id.as_str()) {
                                this.highlighted_event_id = None;
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                }
            })
            .into_any_element()
    } else {
        preview.into_any_element()
    }
}

fn render_edited_indicator(theme: &OneDarkTheme) -> AnyElement {
    div()
        .mt_1()
        .text_xs()
        .italic()
        .text_color(theme.text_muted)
        .child("(edited)")
        .into_any_element()
}

fn render_message_header(
    row_group_id: &SharedString,
    sender_name: &str,
    sender_id: &str,
    timestamp: &str,
    theme: &OneDarkTheme,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .min_w_0()
                .child(
                    div()
                        .group(row_group_id.clone())
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.accent)
                        .cursor_pointer()
                        .hover(|this| this.text_decoration_1().text_decoration_color(theme.accent))
                        .child(sender_name.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(timestamp.to_string()),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .whitespace_nowrap()
                .truncate()
                .max_w(rems(18.0))
                .invisible()
                .group_hover(row_group_id.clone(), |this| this.visible())
                .child(sender_id.to_string()),
        )
        .into_any_element()
}

fn render_message_stack(
    item_id: &str,
    body: &RenderedBody,
    reply_to: Option<&RenderedReplyPreview>,
    reply_interaction: Option<ReplyPreviewInteraction>,
    edited: bool,
    is_notice: bool,
    theme: &OneDarkTheme,
) -> AnyElement {
    div()
        .flex_col()
        .gap_1()
        .when_some(reply_to, |this, reply| {
            this.child(render_reply_preview(
                reply,
                reply_interaction.clone(),
                theme,
            ))
        })
        .child(render_body(item_id, body, is_notice, theme))
        .when(edited, |this| this.child(render_edited_indicator(theme)))
        .into_any_element()
}

fn render_image_stack(
    item_id: &str,
    url: &str,
    caption: Option<&RenderedBody>,
    reply_to: Option<&RenderedReplyPreview>,
    reply_interaction: Option<ReplyPreviewInteraction>,
    edited: bool,
    theme: &OneDarkTheme,
) -> AnyElement {
    div()
        .flex_col()
        .gap_2()
        .when_some(reply_to, |this, reply| {
            this.child(render_reply_preview(
                reply,
                reply_interaction.clone(),
                theme,
            ))
        })
        .child(
            div()
                .rounded_lg()
                .overflow_hidden()
                .border_1()
                .border_color(theme.border)
                .max_w(rems(28.0))
                .max_h(rems(20.0))
                .child(
                    crate::components::remote_image::RemoteImage::new(url.to_string())
                        .object_fit(ObjectFit::ScaleDown)
                        .into_any_element(),
                ),
        )
        .when_some(caption, |this, caption| {
            this.child(render_body(item_id, caption, false, theme))
        })
        .when(edited, |this| this.child(render_edited_indicator(theme)))
        .into_any_element()
}

fn render_bubble_message(
    item_id: &str,
    row_group_id: &SharedString,
    sender_id: &str,
    sender_name: &str,
    body: &RenderedBody,
    reply_to: Option<&RenderedReplyPreview>,
    reply_interaction: Option<ReplyPreviewInteraction>,
    timestamp: &str,
    is_own: bool,
    is_grouped: bool,
    edited: bool,
    is_notice: bool,
    theme: &OneDarkTheme,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .map(|this| if is_own { this.items_end() } else { this })
        .max_w(rems(46.0))
        .py_2()
        .gap_1()
        .when(!is_grouped, |this| {
            this.child(render_message_header(
                row_group_id,
                sender_name,
                sender_id,
                timestamp,
                theme,
            ))
        })
        .child(
            div()
                .px_3()
                .py_2()
                .rounded_lg()
                .bg(if is_own {
                    theme.accent.opacity(0.2)
                } else {
                    theme.sidebar_background
                })
                .border_1()
                .border_color(theme.border)
                .child(render_message_stack(
                    item_id,
                    body,
                    reply_to,
                    reply_interaction,
                    edited,
                    is_notice,
                    theme,
                )),
        )
        .into_any_element()
}

fn render_bubble_image(
    item_id: &str,
    row_group_id: &SharedString,
    sender_id: &str,
    sender_name: &str,
    url: &str,
    caption: Option<&RenderedBody>,
    reply_to: Option<&RenderedReplyPreview>,
    reply_interaction: Option<ReplyPreviewInteraction>,
    timestamp: &str,
    is_own: bool,
    is_grouped: bool,
    edited: bool,
    theme: &OneDarkTheme,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .map(|this| if is_own { this.items_end() } else { this })
        .max_w(rems(46.0))
        .py_2()
        .gap_1()
        .when(!is_grouped, |this| {
            this.child(render_message_header(
                row_group_id,
                sender_name,
                sender_id,
                timestamp,
                theme,
            ))
        })
        .child(
            div()
                .px_3()
                .py_2()
                .rounded_lg()
                .bg(if is_own {
                    theme.accent.opacity(0.2)
                } else {
                    theme.sidebar_background
                })
                .border_1()
                .border_color(theme.border)
                .child(render_image_stack(
                    item_id,
                    url,
                    caption,
                    reply_to,
                    reply_interaction,
                    edited,
                    theme,
                )),
        )
        .into_any_element()
}

fn render_modern_message(
    item_id: &str,
    row_group_id: &SharedString,
    sender_id: &str,
    sender_name: &str,
    body: &RenderedBody,
    reply_to: Option<&RenderedReplyPreview>,
    reply_interaction: Option<ReplyPreviewInteraction>,
    timestamp: &str,
    is_grouped: bool,
    avatar_url: Option<&String>,
    edited: bool,
    is_notice: bool,
    theme: &OneDarkTheme,
    cx: &App,
) -> AnyElement {
    div()
        .hover(|this| this.bg(theme.sidebar_item_hover.opacity(0.5)))
        .px_4()
        .py_1()
        .child(
            div()
                .flex()
                .items_start()
                .gap_4()
                .child(if !is_grouped {
                    render_avatar(avatar_url, sender_name, sender_id, px(40.0), theme, cx)
                } else {
                    div().size_10().flex_shrink_0().into_any_element()
                })
                .child(
                    div()
                        .flex_1()
                        .flex_col()
                        .gap_1()
                        .child(if !is_grouped {
                            render_message_header(
                                row_group_id,
                                sender_name,
                                sender_id,
                                timestamp,
                                theme,
                            )
                        } else {
                            div().into_any_element()
                        })
                        .child(render_message_stack(
                            item_id,
                            body,
                            reply_to,
                            reply_interaction,
                            edited,
                            is_notice,
                            theme,
                        )),
                ),
        )
        .into_any_element()
}

fn render_modern_image(
    item_id: &str,
    row_group_id: &SharedString,
    sender_id: &str,
    sender_name: &str,
    url: &str,
    caption: Option<&RenderedBody>,
    reply_to: Option<&RenderedReplyPreview>,
    reply_interaction: Option<ReplyPreviewInteraction>,
    timestamp: &str,
    is_grouped: bool,
    avatar_url: Option<&String>,
    edited: bool,
    theme: &OneDarkTheme,
    cx: &App,
) -> AnyElement {
    div()
        .hover(|this| this.bg(theme.sidebar_item_hover.opacity(0.5)))
        .px_4()
        .py_1()
        .child(
            div()
                .flex()
                .items_start()
                .gap_4()
                .child(if !is_grouped {
                    render_avatar(avatar_url, sender_name, sender_id, px(40.0), theme, cx)
                } else {
                    div().size_10().flex_shrink_0().into_any_element()
                })
                .child(
                    div()
                        .flex_1()
                        .flex_col()
                        .gap_1()
                        .child(if !is_grouped {
                            render_message_header(
                                row_group_id,
                                sender_name,
                                sender_id,
                                timestamp,
                                theme,
                            )
                        } else {
                            div().into_any_element()
                        })
                        .child(render_image_stack(
                            item_id,
                            url,
                            caption,
                            reply_to,
                            reply_interaction,
                            edited,
                            theme,
                        )),
                ),
        )
        .into_any_element()
}

fn render_system_row(content: &str, timestamp: &str, theme: &OneDarkTheme) -> AnyElement {
    div()
        .px_5()
        .py_1()
        .child(
            div()
                .flex()
                .items_start()
                .gap_3()
                .text_xs()
                .child(div().text_color(theme.text_muted).child("<-"))
                .child(
                    div()
                        .flex_1()
                        .text_color(theme.text_muted)
                        .child(content.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .whitespace_nowrap()
                        .child(timestamp.to_string()),
                ),
        )
        .into_any_element()
}

fn render_centered_divider(text: &str, theme: &OneDarkTheme) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_4()
        .px_4()
        .py_4()
        .child(div().h_px().flex_1().bg(theme.border.opacity(0.8)))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_muted)
                .child(text.to_string()),
        )
        .child(div().h_px().flex_1().bg(theme.border.opacity(0.8)))
        .into_any_element()
}

fn render_timeline_intro(
    room_name: &str,
    avatar_url: Option<&String>,
    theme: &OneDarkTheme,
    cx: &App,
) -> AnyElement {
    div()
        .px_6()
        .pt_6()
        .pb_3()
        .child(
            div()
                .flex_col()
                .gap_4()
                .child(render_avatar(
                    avatar_url,
                    room_name,
                    room_name,
                    px(64.0),
                    theme,
                    cx,
                ))
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text)
                        .child(room_name.to_string()),
                )
                .child(div().text_sm().text_color(theme.text_muted).child(format!(
                    "This is the beginning of your message history in {room_name}."
                )))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child("Older history will appear above when it is available."),
                ),
        )
        .into_any_element()
}

fn render_call_group(
    id: &str,
    sender_name: &str,
    avatar_url: Option<&String>,
    label: &str,
    entries: &[RenderedCallEntry],
    expanded: bool,
    view: Entity<TimelineView>,
    theme: &OneDarkTheme,
    cx: &App,
) -> AnyElement {
    let row_group_id = SharedString::from(format!("call-group-{id}"));
    let id = id.to_string();
    let count_label = format!("{}x", entries.len());
    let entry_rows = entries.iter().map(|entry| {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .px_3()
            .py_1()
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child(entry.content.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .whitespace_nowrap()
                    .child(entry.timestamp.clone()),
            )
            .into_any_element()
    });

    div()
        .group(row_group_id.clone())
        .px_4()
        .py_2()
        .child(
            div()
                .rounded_lg()
                .bg(theme.sidebar_background.opacity(0.55))
                .border_1()
                .border_color(theme.border.opacity(0.5))
                .child(
                    div()
                        .px_3()
                        .py_3()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(render_avatar(
                            avatar_url,
                            sender_name,
                            sender_name,
                            px(32.0),
                            theme,
                            cx,
                        ))
                        .child(
                            div()
                                .flex_1()
                                .flex_col()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.text)
                                        .child(sender_name.to_string()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child(label.to_string()),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child(count_label),
                        )
                        .child(
                            div()
                                .cursor_pointer()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .hover(|this| this.text_color(theme.text))
                                .child(if expanded { "v" } else { ">" })
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.toggle_call_group(&id, cx);
                                    });
                                }),
                        ),
                )
                .when(expanded, |this| {
                    this.child(
                        div()
                            .border_t_1()
                            .border_color(theme.border.opacity(0.5))
                            .children(entry_rows),
                    )
                }),
        )
        .into_any_element()
}

impl Render for TimelineView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model_handle = self.model.clone();
        let (room, rendered_items, list_state, chat_style, loading_history, hit_timeline_start) = {
            let model_read = model_handle.read(cx);
            (
                model_read.room.clone(),
                model_read.rendered_items.clone(),
                model_read.list_state.clone(),
                model_read.chat_style,
                model_read.loading_history,
                model_read.hit_timeline_start,
            )
        };
        let has_rendered_items = !rendered_items.is_empty();
        let show_start_of_conversation = hit_timeline_start && has_rendered_items;
        let intro_row_count = if show_start_of_conversation { 2 } else { 0 };
        let total_row_count = rendered_items.len() + intro_row_count;

        if list_state.item_count() != total_row_count {
            list_state.reset(total_row_count);
        }

        tracing::info!(
            "timeline_view: rendering {} items with list_count={}",
            rendered_items.len(),
            list_state.item_count()
        );
        let theme = *cx.onedark_theme();
        let highlighted_event_id = self.highlighted_event_id.clone();
        let expanded_call_groups = self.expanded_call_groups.clone();
        let room_name = room
            .cached_display_name()
            .map(|name| name.to_string())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "Conversation".to_string());
        let room_avatar_url = room.avatar_url().map(|url| url.to_string());
        let rendered_items_for_list = rendered_items.clone();
        let event_indices = Arc::new(
            rendered_items
                .iter()
                .enumerate()
                .filter_map(|(ix, item)| {
                    rendered_item_id(item).map(|id| (id.to_string(), ix + intro_row_count))
                })
                .collect::<HashMap<_, _>>(),
        );
        let view = cx.entity().clone();
        let list_state_for_rows = list_state.clone();
        let room_name_for_rows = room_name.clone();
        let room_avatar_url_for_rows = room_avatar_url.clone();

        let timeline_list = list(list_state.clone(), move |ix, _window, cx| {
            if show_start_of_conversation {
                if ix == 0 {
                    return render_timeline_intro(
                        &room_name_for_rows,
                        room_avatar_url_for_rows.as_ref(),
                        cx.onedark_theme(),
                        cx,
                    );
                }

                if ix == 1 {
                    return render_centered_divider("Start of messages", cx.onedark_theme());
                }
            }

            let item_ix = ix.saturating_sub(intro_row_count);
            let Some(item) = rendered_items_for_list.get(item_ix) else {
                return div()
                    .px_4()
                    .py_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.onedark_theme().text_muted)
                            .child("Loading..."),
                    )
                    .into_any_element();
            };
            let theme = *cx.onedark_theme();
            let row_group_id = SharedString::from(format!("timeline-row-{item_ix}"));
            let highlighted = rendered_item_id(item)
                .and_then(|id| {
                    highlighted_event_id
                        .as_deref()
                        .map(|selected| selected == id)
                })
                .unwrap_or(false);

            let row = match item {
                RenderedTimelineItem::Message {
                    id,
                    sender_id,
                    sender_name,
                    body,
                    timestamp,
                    is_own,
                    is_grouped,
                    avatar_url,
                    edited,
                    is_notice,
                    reply_to,
                } => match chat_style {
                    ChatStyle::Bubble => render_bubble_message(
                        id,
                        &row_group_id,
                        sender_id,
                        sender_name,
                        body,
                        reply_to.as_ref(),
                        reply_to.as_ref().map(|reply| ReplyPreviewInteraction {
                            target_event_id: reply.event_id.clone(),
                            event_indices: event_indices.clone(),
                            list_state: list_state_for_rows.clone(),
                            view: view.clone(),
                        }),
                        timestamp,
                        *is_own,
                        *is_grouped,
                        *edited,
                        *is_notice,
                        &theme,
                    ),
                    ChatStyle::Modern => render_modern_message(
                        id,
                        &row_group_id,
                        sender_id,
                        sender_name,
                        body,
                        reply_to.as_ref(),
                        reply_to.as_ref().map(|reply| ReplyPreviewInteraction {
                            target_event_id: reply.event_id.clone(),
                            event_indices: event_indices.clone(),
                            list_state: list_state_for_rows.clone(),
                            view: view.clone(),
                        }),
                        timestamp,
                        *is_grouped,
                        avatar_url.as_ref(),
                        *edited,
                        *is_notice,
                        &theme,
                        cx,
                    ),
                },
                RenderedTimelineItem::Image {
                    id,
                    sender_id,
                    sender_name,
                    url,
                    caption,
                    timestamp,
                    is_own,
                    is_grouped,
                    avatar_url,
                    edited,
                    reply_to,
                } => match chat_style {
                    ChatStyle::Bubble => render_bubble_image(
                        id,
                        &row_group_id,
                        sender_id,
                        sender_name,
                        url,
                        caption.as_ref(),
                        reply_to.as_ref(),
                        reply_to.as_ref().map(|reply| ReplyPreviewInteraction {
                            target_event_id: reply.event_id.clone(),
                            event_indices: event_indices.clone(),
                            list_state: list_state_for_rows.clone(),
                            view: view.clone(),
                        }),
                        timestamp,
                        *is_own,
                        *is_grouped,
                        *edited,
                        &theme,
                    ),
                    ChatStyle::Modern => render_modern_image(
                        id,
                        &row_group_id,
                        sender_id,
                        sender_name,
                        url,
                        caption.as_ref(),
                        reply_to.as_ref(),
                        reply_to.as_ref().map(|reply| ReplyPreviewInteraction {
                            target_event_id: reply.event_id.clone(),
                            event_indices: event_indices.clone(),
                            list_state: list_state_for_rows.clone(),
                            view: view.clone(),
                        }),
                        timestamp,
                        *is_grouped,
                        avatar_url.as_ref(),
                        *edited,
                        &theme,
                        cx,
                    ),
                },
                RenderedTimelineItem::System {
                    content, timestamp, ..
                } => render_system_row(content, timestamp, &theme),
                RenderedTimelineItem::CallGroup {
                    id,
                    sender_name,
                    avatar_url,
                    label,
                    entries,
                } => render_call_group(
                    id,
                    sender_name,
                    avatar_url.as_ref(),
                    label,
                    entries,
                    expanded_call_groups.contains(id),
                    view.clone(),
                    &theme,
                    cx,
                ),
                RenderedTimelineItem::Separator(text) => render_centered_divider(text, &theme),
            };

            div()
                .bg(if highlighted {
                    theme.accent.opacity(0.14)
                } else {
                    transparent_black()
                })
                .child(row)
                .into_any_element()
        })
        .size_full()
        .map(|this| if loading_history { this.pt_10() } else { this });

        div()
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_hidden()
            .child(if !has_rendered_items {
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.text_muted)
                            .child("Syncing timeline..."),
                    )
                    .into_any_element()
            } else {
                timeline_list.into_any_element()
            })
            .when(has_rendered_items, |this| {
                this.child(Scrollbar::vertical(&list_state))
            })
            .when(loading_history, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right(px(10.0))
                        .h_10()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(theme.background.opacity(0.94))
                        .border_b_1()
                        .border_color(theme.border.opacity(0.6))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("Loading older messages..."),
                        ),
                )
            })
    }
}
