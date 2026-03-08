use crate::theme::onedark::OneDarkTheme;
use crate::timeline::RenderedBody;
use gpui::*;
use gpui_component::clipboard::Clipboard;

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

pub(super) fn render_body(
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
