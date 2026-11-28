//! Преобразование Markdown в строки терминала без HTML, выполнения кода и открытия URL.
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use tvision::color::*;

/// Семантическое выделение. Терминальный Attr поддерживает цвета, но не шрифты:
/// код, заголовки, сильное выделение, курсив и ссылки различаются цветом.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style(u8);
impl Style {
    const CODE: u8 = 1;
    const STRONG: u8 = 2;
    const EMPHASIS: u8 = 4;
    const HEADING: u8 = 8;
    const LINK: u8 = 16;
    /// Применяет выделение, сохраняя фон подсказки и цветовую глубину темы.
    pub fn attr(self, normal: Attr, wide: bool) -> Attr {
        let (ansi, indexed) = if self.0 & Self::CODE != 0 {
            (4, 180)
        } else if self.0 & Self::HEADING != 0 {
            (1, 81)
        } else if self.0 & Self::STRONG != 0 {
            (0, 231)
        } else if self.0 & Self::LINK != 0 {
            (1, 117)
        } else if self.0 & Self::EMPHASIS != 0 {
            (5, 213)
        } else {
            return normal;
        };
        if wide {
            with_fg256(normal, indexed)
        } else if self.0 & Self::STRONG != 0 {
            attr(15, 8)
        } else {
            with_fore(normal, ansi)
        }
    }
}

type Row = Vec<(char, Style)>;
/// Накопитель строк считает экранные ячейки, не байты. Табуляция разворачивается
/// до следующей позиции, управляющие символы не попадают в терминал.
struct Rows {
    rows: Vec<Row>,
    width: usize,
    used: usize,
}
impl Rows {
    fn new(width: usize) -> Self {
        Self { rows: vec![vec![]], width: width.max(1), used: 0 }
    }
    fn newline(&mut self) {
        self.rows.push(vec![]);
        self.used = 0;
    }
    fn finish_line(&mut self) {
        if self.used > 0 {
            self.newline();
        }
    }
    fn push(&mut self, text: &str, style: Style) {
        for ch in text.chars() {
            if ch == '\n' {
                self.newline();
                continue;
            }
            if ch == '\t' {
                self.push(&" ".repeat(4 - self.used % 4), style);
                continue;
            }
            let mut ch = if ch.is_control() { ' ' } else { ch };
            let mut cells = tvision::text::char_width(ch).max(1);
            if cells > self.width {
                ch = '?';
                cells = 1;
            }
            if self.used + cells > self.width {
                self.newline();
            }
            self.rows.last_mut().unwrap().push((ch, style));
            self.used += cells;
        }
    }
}
/// Возвращает переносимые строки со стилями; plaintext обходит парсер полностью.
/// Ссылки выводятся вместе с адресом. HTML остаётся буквальным текстом.
pub(super) fn render(text: &str, markdown: bool, width: usize) -> Vec<Row> {
    let mut out = Rows::new(width);
    if !markdown {
        out.push(text, Style::default());
    } else {
        let mut style = Style::default();
        let mut stack = Vec::new();
        let mut lists: Vec<Option<u64>> = Vec::new();
        let mut links = Vec::new();
        for event in Parser::new(text) {
            match event {
                Event::Start(tag) => {
                    stack.push(style);
                    match tag {
                        Tag::Strong => style.0 |= Style::STRONG,
                        Tag::Emphasis => style.0 |= Style::EMPHASIS,
                        Tag::Heading { .. } => {
                            out.finish_line();
                            style.0 |= Style::HEADING;
                        }
                        Tag::CodeBlock(_) => {
                            out.finish_line();
                            style.0 |= Style::CODE;
                        }
                        Tag::List(start) => {
                            out.finish_line();
                            lists.push(start);
                        }
                        Tag::Item => {
                            out.finish_line();
                            let indent = "  ".repeat(lists.len().saturating_sub(1));
                            let marker = match lists.last_mut() {
                                Some(Some(n)) => {
                                    let s = format!("{n}. ");
                                    *n = n.saturating_add(1);
                                    s
                                }
                                _ => "• ".into(),
                            };
                            out.push(&format!("{indent}{marker}"), style);
                        }
                        Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                            links.push(dest_url);
                            style.0 |= Style::LINK;
                        }
                        Tag::BlockQuote(_) => {
                            out.finish_line();
                            out.push("│ ", style);
                        }
                        _ => {}
                    }
                }
                Event::End(tag) => {
                    match tag {
                        TagEnd::Link | TagEnd::Image => {
                            if let Some(url) = links.pop() {
                                out.push(&format!(" ({url})"), style);
                            }
                        }
                        TagEnd::List(_) => {
                            lists.pop();
                            out.finish_line();
                        }
                        TagEnd::Paragraph | TagEnd::Heading(_) => {
                            out.finish_line();
                            if lists.is_empty() {
                                out.newline();
                            }
                        }
                        TagEnd::Item | TagEnd::CodeBlock | TagEnd::BlockQuote(_) => out.finish_line(),
                        _ => {}
                    }
                    style = stack.pop().unwrap_or_default();
                }
                Event::Text(s) | Event::Html(s) | Event::InlineHtml(s) => out.push(&s, style),
                Event::Code(s) => out.push(&s, Style(style.0 | Style::CODE)),
                Event::SoftBreak => out.push(" ", style),
                Event::HardBreak => out.newline(),
                Event::Rule => {
                    out.finish_line();
                    out.push(&"─".repeat(width), style);
                    out.newline();
                }
                _ => {}
            }
        }
    }
    while out.rows.last().is_some_and(Vec::is_empty) {
        out.rows.pop();
    }
    out.rows
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text(rows: &[Row]) -> String {
        rows.iter().map(|r| r.iter().map(|(c, _)| c).collect::<String>()).collect::<Vec<_>>().join("\n")
    }
    #[test]
    fn markdown_blocks_inline_and_links() {
        let rows = render(
            "# Title\n\n**bold** *em* `code` [docs](https://example.org)\n\n- item\n  - nested\n\n3. third\n\n```rust\nlet x = **literal**;\n```",
            true,
            100,
        );
        let rendered = text(&rows);
        for s in ["Title", "bold em code docs (https://example.org)", "• item", "  • nested", "3. third", "let x = **literal**;"] {
            assert!(rendered.contains(s), "{rendered}");
        }
        for mask in [Style::CODE, Style::STRONG, Style::EMPHASIS, Style::HEADING, Style::LINK] {
            assert!(rows.iter().flatten().any(|(_, s)| s.0 & mask != 0));
        }
        assert!(!rendered.contains("```"));
    }
    #[test]
    fn plaintext_preserves_markers_and_unicode_fits() {
        let input = "# title\n**bold** `code` [link](url)\n```\n变量\txy\n```";
        let rows = render(input, false, 80);
        assert_eq!(text(&rows), input.replace('\t', "    "));
        assert!(rows.iter().flatten().all(|(_, s)| *s == Style::default()));
        for width in 1..10 {
            for row in render("**变量** `very_long_code`\n\n```\n\t变量\u{1b}[31m\n```", true, width) {
                assert!(row.iter().map(|(c, _)| tvision::text::char_width(*c).max(1)).sum::<usize>() <= width);
                assert!(!row.iter().any(|(c, _)| c.is_control()));
            }
        }
    }
}
