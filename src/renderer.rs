use ratatui::style::{Color, Modifier, Style};
use url::Url;

use crate::browser::{LineType, PageLink, RenderedPage, StyledLine, StyledSpan};
use crate::parser::{DomNode, Element, Tag};

// ── Public entry point ───────────────────────────────────────────────────────

pub fn render(root: &DomNode, base_url: &Url, col_width: u16) -> RenderedPage {
    let mut ctx = RenderContext::new(col_width, base_url.clone());
    ctx.walk(root);
    ctx.flush_inline();

    RenderedPage {
        url: base_url.clone(),
        title: String::new(), // filled in by caller from ParseResult.title
        lines: ctx.lines,
        links: ctx.links,
        focused_link: None,
    }
}

// ── Render context ────────────────────────────────────────────────────────────

struct RenderContext {
    col_width: u16,
    base_url: Url,

    lines: Vec<StyledLine>,
    links: Vec<PageLink>,

    // Inline span accumulator
    inline_spans: Vec<StyledSpan>,
    current_line_type: LineType,

    // Style stack
    style_stack: Vec<StyleFrame>,

    // List state
    list_depth: u8,
    list_counters: Vec<usize>, // for ol
    in_ol: Vec<bool>,

    // Pre/code block
    in_pre: bool,

    // Whether last emitted line was blank (to avoid double blanks)
    last_was_blank: bool,
}

#[derive(Clone)]
struct StyleFrame {
    style: Style,
    is_link: Option<usize>, // link index if inside <a>
}

impl RenderContext {
    fn new(col_width: u16, base_url: Url) -> Self {
        Self {
            col_width,
            base_url,
            lines: Vec::new(),
            links: Vec::new(),
            inline_spans: Vec::new(),
            current_line_type: LineType::Normal,
            style_stack: vec![StyleFrame { style: Style::default(), is_link: None }],
            list_depth: 0,
            list_counters: Vec::new(),
            in_ol: Vec::new(),
            in_pre: false,
            last_was_blank: false,
        }
    }

    fn current_style(&self) -> Style {
        self.style_stack.last().unwrap().style
    }

    fn current_link(&self) -> Option<usize> {
        self.style_stack.iter().rev().find_map(|f| f.is_link)
    }

    fn push_style(&mut self, style: Style, link: Option<usize>) {
        let merged = self.current_style().patch(style);
        self.style_stack.push(StyleFrame { style: merged, is_link: link });
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn flush_inline(&mut self) {
        if self.inline_spans.is_empty() { return; }
        let spans = std::mem::take(&mut self.inline_spans);
        let line_type = std::mem::replace(&mut self.current_line_type, LineType::Normal);
        // Remove leading/trailing space-only spans from normal lines
        let line = StyledLine { spans, line_type };
        self.push_line(line);
    }

    fn push_line(&mut self, line: StyledLine) {
        let is_blank = line.spans.is_empty()
            || line.spans.iter().all(|s| s.text.trim().is_empty());
        if is_blank && self.last_was_blank {
            return; // deduplicate blank lines
        }
        self.last_was_blank = is_blank;
        self.lines.push(line);
    }

    fn push_blank_lines(&mut self, n: u16) {
        for _ in 0..n {
            if !self.last_was_blank {
                self.push_line(StyledLine::empty());
            }
        }
    }

    fn indent_str(&self) -> String {
        "  ".repeat(self.list_depth as usize)
    }

    fn add_span(&mut self, text: String, style: Style, link_idx: Option<usize>) {
        if text.is_empty() { return; }
        self.inline_spans.push(StyledSpan { text, style, link_idx });
    }

    // ── Walk ─────────────────────────────────────────────────────────────────

    fn walk(&mut self, node: &DomNode) {
        match node {
            DomNode::Document(children) | DomNode::Element(Element { tag: Tag::Html | Tag::Body | Tag::Head, children, .. }) => {
                for child in children { self.walk(child); }
            }

            DomNode::Element(el) => self.walk_element(el),

            DomNode::Text(text) => self.walk_text(text),

            DomNode::Image(img) => {
                let line_idx = self.lines.len() + if self.inline_spans.is_empty() { 0 } else { 1 };
                self.flush_inline();
                self.lines.push(StyledLine {
                    spans: vec![StyledSpan {
                        text: if img.alt.is_empty() {
                            format!("[image: {}]", img.src)
                        } else {
                            format!("[{}]", img.alt)
                        },
                        style: Style::default().fg(Color::DarkGray),
                        link_idx: None,
                    }],
                    line_type: LineType::ImagePlaceholder {
                        chafa_output: None,
                        alt: img.alt.clone(),
                    },
                });
            }
        }
    }

    fn walk_text(&mut self, text: &str) {
        if self.in_pre {
            // Preserve whitespace; split on newlines
            for (i, line) in text.split('\n').enumerate() {
                if i > 0 { self.flush_inline(); }
                self.add_span(line.to_string(), self.current_style(), self.current_link());
            }
        } else {
            // Word-wrap to col_width
            let indent = self.indent_str();
            let effective_width = (self.col_width as usize).saturating_sub(indent.len()).max(20);
            let opts = textwrap::Options::new(effective_width);
            let wrapped = textwrap::wrap(text.trim(), opts);
            for (i, line) in wrapped.iter().enumerate() {
                if i > 0 {
                    self.flush_inline();
                    if !indent.is_empty() {
                        self.add_span(indent.clone(), Style::default(), None);
                    }
                }
                if i == 0 && !self.inline_spans.is_empty() {
                    // Append to existing line — just add the span
                    self.add_span(line.to_string(), self.current_style(), self.current_link());
                } else {
                    self.add_span(line.to_string(), self.current_style(), self.current_link());
                }
            }
        }
    }

    fn walk_element(&mut self, el: &Element) {
        match &el.tag {
            // ── Stripped ─────────────────────────────────────────────────────
            Tag::Script | Tag::Style | Tag::Noscript => {}

            // ── Line break ───────────────────────────────────────────────────
            Tag::Br => {
                self.flush_inline();
            }

            // ── Horizontal rule ──────────────────────────────────────────────
            Tag::Hr => {
                self.flush_inline();
                let rule = "─".repeat(self.col_width as usize);
                self.push_line(StyledLine {
                    spans: vec![StyledSpan {
                        text: rule,
                        style: Style::default().fg(Color::DarkGray),
                        link_idx: None,
                    }],
                    line_type: LineType::HorizontalRule,
                });
            }

            // ── Headings ─────────────────────────────────────────────────────
            Tag::H1 | Tag::H2 | Tag::H3 | Tag::H4 | Tag::H5 | Tag::H6 => {
                self.flush_inline();
                self.push_blank_lines(el.style.margin_top);

                let level = match &el.tag {
                    Tag::H1 => 1u8, Tag::H2 => 2, Tag::H3 => 3,
                    Tag::H4 => 4, Tag::H5 => 5, _ => 6,
                };
                let style = Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(el.style.color.unwrap_or(Color::White));

                self.current_line_type = LineType::Heading(level);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
                self.flush_inline();

                // Underline separator for h1/h2
                if level <= 2 {
                    let sep_char = if level == 1 { "═" } else { "─" };
                    let sep = sep_char.repeat(self.col_width as usize);
                    self.push_line(StyledLine {
                        spans: vec![StyledSpan {
                            text: sep,
                            style: Style::default().fg(el.style.color.unwrap_or(Color::DarkGray)),
                            link_idx: None,
                        }],
                        line_type: LineType::Normal,
                    });
                }

                self.push_blank_lines(el.style.margin_bottom);
            }

            // ── Paragraph / block elements ───────────────────────────────────
            Tag::P => {
                self.flush_inline();
                self.push_blank_lines(1);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.push_blank_lines(1);
            }

            Tag::Div | Tag::Section | Tag::Article | Tag::Main
            | Tag::Nav | Tag::Aside | Tag::Header | Tag::Footer => {
                self.flush_inline();
                for child in &el.children { self.walk(child); }
                self.flush_inline();
            }

            // ── Blockquote ───────────────────────────────────────────────────
            Tag::Blockquote => {
                self.flush_inline();
                self.push_blank_lines(1);
                let style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC);
                self.push_style(style, None);

                // Add a ▌ prefix to each line
                self.add_span("▌ ".to_string(), style, None);
                for child in &el.children { self.walk(child); }
                self.flush_inline();

                self.pop_style();
                self.push_blank_lines(1);
            }

            // ── Pre / Code ───────────────────────────────────────────────────
            Tag::Pre => {
                self.flush_inline();
                self.push_blank_lines(1);
                let style = Style::default().fg(Color::Green);
                self.push_style(style, None);
                self.in_pre = true;
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.in_pre = false;
                self.pop_style();
                self.push_blank_lines(1);
            }

            Tag::Code => {
                let style = Style::default().fg(Color::Green);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
            }

            // ── Lists ────────────────────────────────────────────────────────
            Tag::Ul => {
                self.flush_inline();
                self.list_depth += 1;
                self.in_ol.push(false);
                self.list_counters.push(0);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.list_depth -= 1;
                self.in_ol.pop();
                self.list_counters.pop();
            }

            Tag::Ol => {
                self.flush_inline();
                self.list_depth += 1;
                self.in_ol.push(true);
                self.list_counters.push(0);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.list_depth -= 1;
                self.in_ol.pop();
                self.list_counters.pop();
            }

            Tag::Li => {
                self.flush_inline();
                let indent = self.indent_str();
                let is_ol = self.in_ol.last().copied().unwrap_or(false);
                let bullet = if is_ol {
                    if let Some(counter) = self.list_counters.last_mut() {
                        *counter += 1;
                        format!("{}{}. ", indent, counter)
                    } else {
                        format!("{}• ", indent)
                    }
                } else {
                    format!("{}• ", indent)
                };

                self.add_span(bullet, Style::default().fg(Color::Yellow), None);
                self.current_line_type = LineType::ListItem(self.list_depth);

                for child in &el.children { self.walk(child); }
                self.flush_inline();
            }

            // ── Table (basic) ────────────────────────────────────────────────
            Tag::Table => {
                self.flush_inline();
                self.push_blank_lines(1);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.push_blank_lines(1);
            }

            Tag::THead | Tag::TBody | Tag::TFoot => {
                for child in &el.children { self.walk(child); }
            }

            Tag::Tr => {
                self.flush_inline();
                self.add_span("│ ".to_string(), Style::default().fg(Color::DarkGray), None);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
            }

            Tag::Td => {
                for child in &el.children { self.walk(child); }
                self.add_span(" │ ".to_string(), Style::default().fg(Color::DarkGray), None);
            }

            Tag::Th => {
                let style = Style::default().add_modifier(Modifier::BOLD);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
                self.add_span(" │ ".to_string(), Style::default().fg(Color::DarkGray), None);
            }

            // ── Anchor ───────────────────────────────────────────────────────
            Tag::A => {
                let href = el.attrs.get("href").cloned().unwrap_or_default();
                let resolved = if !href.is_empty() {
                    self.base_url.join(&href).ok()
                        .map(|u| u.to_string())
                        .unwrap_or(href.clone())
                } else {
                    href.clone()
                };

                // Register link before walking children (to capture text)
                let link_idx = self.links.len();
                self.links.push(PageLink {
                    line_idx: self.lines.len(),
                    col_range: 0..0,  // updated after render
                    href: resolved,
                    text: String::new(), // filled retroactively
                });

                let style = Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::UNDERLINED);
                self.push_style(style, Some(link_idx));

                let span_start = self.inline_spans.len();
                for child in &el.children { self.walk(child); }

                // Capture link text
                let link_text: String = self.inline_spans[span_start..]
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<Vec<_>>()
                    .join("");
                if let Some(link) = self.links.get_mut(link_idx) {
                    link.text = link_text;
                    link.line_idx = self.lines.len();
                }

                self.pop_style();
            }

            // ── Strong / Em / B / I ──────────────────────────────────────────
            Tag::Strong | Tag::B => {
                let style = Style::default().add_modifier(Modifier::BOLD);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
            }

            Tag::Em | Tag::I => {
                let style = Style::default().add_modifier(Modifier::ITALIC);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
            }

            // ── Span and unknown inlines ──────────────────────────────────────
            Tag::Span | Tag::Unknown(_) => {
                for child in &el.children { self.walk(child); }
            }

            // ── Img (handled in walk() as DomNode::Image) ────────────────────
            Tag::Img => {}

            // ── Remaining block containers ────────────────────────────────────
            _ => {
                if el.tag.is_block() {
                    self.flush_inline();
                }
                for child in &el.children { self.walk(child); }
                if el.tag.is_block() {
                    self.flush_inline();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn render_html(html: &str) -> RenderedPage {
        let base = Url::parse("http://example.com").unwrap();
        let parsed = parser::parse(html, &base);
        render(&parsed.root, &base, 80)
    }

    #[test]
    fn test_render_heading() {
        let page = render_html("<html><body><h1>Hello World</h1></body></html>");
        let heading_lines: Vec<_> = page.lines.iter()
            .filter(|l| matches!(l.line_type, LineType::Heading(1)))
            .collect();
        assert!(!heading_lines.is_empty());
        let text: String = heading_lines[0].spans.iter().map(|s| s.text.as_str()).collect();
        assert!(text.contains("Hello World"));
    }

    #[test]
    fn test_render_link() {
        let page = render_html(r#"<html><body><a href="/about">About</a></body></html>"#);
        assert!(!page.links.is_empty());
        assert_eq!(page.links[0].text, "About");
    }
}
