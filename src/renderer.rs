use ratatui::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthStr;
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
        title: String::new(),
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

    inline_spans: Vec<StyledSpan>,
    current_line_type: LineType,

    style_stack: Vec<StyleFrame>,

    list_depth: u8,
    list_counters: Vec<usize>,
    in_ol: Vec<bool>,

    in_pre: bool,

    last_was_blank: bool,
}

#[derive(Clone)]
struct StyleFrame {
    style: Style,
    is_link: Option<usize>,
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

        // Update col_range for any links in this line
        let mut col: u16 = 0;
        for span in &spans {
            let w = UnicodeWidthStr::width(span.text.as_str()) as u16;
            if let Some(link_idx) = span.link_idx {
                if let Some(link) = self.links.get_mut(link_idx) {
                    // Only set col_range the first time we see this link
                    if link.col_range == (0..0) {
                        link.col_range = col..col + w;
                        link.line_idx = self.lines.len();
                    }
                }
            }
            col = col.saturating_add(w);
        }

        self.push_line(StyledLine { spans, line_type });
    }

    fn push_line(&mut self, line: StyledLine) {
        let is_blank = line.spans.is_empty()
            || line.spans.iter().all(|s| s.text.trim().is_empty());
        if is_blank && self.last_was_blank {
            return;
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

    fn effective_width(&self) -> usize {
        let indent = self.indent_str().len();
        (self.col_width as usize).saturating_sub(indent).max(20)
    }

    // ── Walk ─────────────────────────────────────────────────────────────────

    fn walk(&mut self, node: &DomNode) {
        match node {
            DomNode::Document(children) => {
                for child in children { self.walk(child); }
            }

            DomNode::Element(Element { tag: Tag::Html | Tag::Body | Tag::Head, children, .. }) => {
                for child in children { self.walk(child); }
            }

            DomNode::Element(el) => self.walk_element(el),

            DomNode::Text(text) => self.walk_text(text),

            DomNode::Image(img) => {
                self.flush_inline();
                let display_text = if img.alt.is_empty() {
                    format!("[IMG: {}]", truncate_url(&img.src, 40))
                } else {
                    format!("[IMG: {}]", img.alt)
                };
                self.push_line(StyledLine {
                    spans: vec![StyledSpan {
                        text: display_text,
                        style: Style::default().fg(Color::Magenta).add_modifier(Modifier::ITALIC),
                        link_idx: None,
                    }],
                    line_type: LineType::ImagePlaceholder {
                        chafa_output: None,
                        alt: img.alt.clone(),
                        src: img.src.clone(),
                    },
                });
            }
        }
    }

    fn walk_text(&mut self, text: &str) {
        if self.in_pre {
            // Preserve whitespace; split on newlines; truncate long lines
            for (i, line) in text.split('\n').enumerate() {
                if i > 0 { self.flush_inline(); }
                if line.is_empty() {
                    self.flush_inline();
                    continue;
                }
                let max = (self.col_width as usize).saturating_sub(1);
                let display = if UnicodeWidthStr::width(line) > max {
                    let mut s = truncate_to_width(line, max.saturating_sub(1));
                    s.push('→');
                    s
                } else {
                    line.to_string()
                };
                self.add_span(display, self.current_style(), self.current_link());
            }
        } else {
            let indent = self.indent_str();
            let effective_width = self.effective_width();
            let opts = textwrap::Options::new(effective_width);
            let wrapped = textwrap::wrap(text.trim(), opts);

            for (i, line) in wrapped.iter().enumerate() {
                if i > 0 {
                    self.flush_inline();
                    if !indent.is_empty() {
                        self.add_span(indent.clone(), Style::default(), None);
                    }
                }
                let s = line.to_string();
                if !s.is_empty() {
                    self.add_span(s, self.current_style(), self.current_link());
                }
            }
        }
    }

    fn walk_element(&mut self, el: &Element) {
        match &el.tag {

            // ── Stripped ─────────────────────────────────────────────────────
            Tag::Script | Tag::Style | Tag::Noscript | Tag::Colgroup | Tag::Col => {}

            // ── Line break ───────────────────────────────────────────────────
            Tag::Br => { self.flush_inline(); }

            // ── Horizontal rule ──────────────────────────────────────────────
            Tag::Hr => {
                self.flush_inline();
                let width = (self.col_width as usize).max(1);
                let rule = "─".repeat(width);
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

                // Underline separators
                let width = (self.col_width as usize).max(1);
                let sep = match level {
                    1 => Some("═".repeat(width)),
                    2 => Some("─".repeat(width)),
                    3 => Some("·".repeat(width)),
                    _ => None,
                };
                if let Some(sep_str) = sep {
                    self.push_line(StyledLine {
                        spans: vec![StyledSpan {
                            text: sep_str,
                            style: Style::default().fg(el.style.color.unwrap_or(Color::DarkGray)),
                            link_idx: None,
                        }],
                        line_type: LineType::Normal,
                    });
                }
                self.push_blank_lines(el.style.margin_bottom);
            }

            // ── Paragraph ────────────────────────────────────────────────────
            Tag::P => {
                self.flush_inline();
                self.push_blank_lines(1);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.push_blank_lines(1);
            }

            // ── Generic block containers ──────────────────────────────────────
            Tag::Div | Tag::Section | Tag::Article | Tag::Main
            | Tag::Nav | Tag::Aside | Tag::Header | Tag::Footer
            | Tag::Figure => {
                self.flush_inline();
                for child in &el.children { self.walk(child); }
                self.flush_inline();
            }

            // ── Address ───────────────────────────────────────────────────────
            Tag::Address => {
                self.flush_inline();
                self.push_blank_lines(1);
                let style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.pop_style();
                self.push_blank_lines(1);
            }

            // ── Blockquote ───────────────────────────────────────────────────
            Tag::Blockquote => {
                self.flush_inline();
                self.push_blank_lines(1);

                let style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC);
                self.push_style(style, None);

                let start_line = self.lines.len();

                for child in &el.children { self.walk(child); }
                self.flush_inline();

                self.pop_style();

                // Prepend ▌ to every line added in the blockquote
                let bq_style = Style::default().fg(Color::Cyan);
                for line in &mut self.lines[start_line..] {
                    line.spans.insert(0, StyledSpan {
                        text: "▌ ".to_string(),
                        style: bq_style,
                        link_idx: None,
                    });
                }

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

            Tag::Samp => {
                let style = Style::default().fg(Color::Green);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
            }

            // ── Definition lists ──────────────────────────────────────────────
            Tag::Dl => {
                self.flush_inline();
                self.push_blank_lines(1);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.push_blank_lines(1);
            }

            Tag::Dt => {
                self.flush_inline();
                let style = Style::default().add_modifier(Modifier::BOLD);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.pop_style();
            }

            Tag::Dd => {
                self.flush_inline();
                self.add_span("  ".to_string(), Style::default(), None);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
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
                    let bullet_char = match self.list_depth {
                        1 => "•",
                        2 => "◦",
                        _ => "▸",
                    };
                    format!("{}{} ", indent, bullet_char)
                };

                self.add_span(bullet, Style::default().fg(Color::Yellow), None);
                self.current_line_type = LineType::ListItem(self.list_depth);

                for child in &el.children { self.walk(child); }
                self.flush_inline();
            }

            // ── Table ────────────────────────────────────────────────────────
            Tag::Table => {
                self.flush_inline();
                self.push_blank_lines(1);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.push_blank_lines(1);
            }

            Tag::Caption => {
                self.flush_inline();
                let style = Style::default().add_modifier(Modifier::BOLD);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.pop_style();
                // Separator below caption
                let width = (self.col_width as usize).max(1);
                self.push_line(StyledLine {
                    spans: vec![StyledSpan {
                        text: "─".repeat(width),
                        style: Style::default().fg(Color::DarkGray),
                        link_idx: None,
                    }],
                    line_type: LineType::Normal,
                });
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
                let style = Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
                self.add_span(" │ ".to_string(), Style::default().fg(Color::DarkGray), None);
            }

            // ── Anchor ───────────────────────────────────────────────────────
            Tag::A => {
                let href = el.attrs.get("href").cloned().unwrap_or_default();
                if href.is_empty() {
                    for child in &el.children { self.walk(child); }
                    return;
                }

                let link_idx = self.links.len();
                self.links.push(PageLink {
                    line_idx: self.lines.len(),
                    col_range: 0..0,
                    href: href.clone(),
                    text: String::new(),
                });

                let style = Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::UNDERLINED);
                self.push_style(style, Some(link_idx));

                let span_start = self.inline_spans.len();
                for child in &el.children { self.walk(child); }

                let link_text: String = self.inline_spans[span_start..]
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<Vec<_>>()
                    .join("");
                if let Some(link) = self.links.get_mut(link_idx) {
                    link.text = link_text;
                    // line_idx and col_range updated in flush_inline
                }

                self.pop_style();
            }

            // ── Inline formatting ─────────────────────────────────────────────
            Tag::Strong | Tag::B => {
                let style = Style::default().add_modifier(Modifier::BOLD);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
            }

            Tag::Em | Tag::I | Tag::Cite => {
                let style = Style::default().add_modifier(Modifier::ITALIC);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
            }

            Tag::Del | Tag::S => {
                let style = Style::default().add_modifier(Modifier::CROSSED_OUT);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
            }

            Tag::Ins | Tag::U => {
                let style = Style::default().add_modifier(Modifier::UNDERLINED);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
            }

            Tag::Mark => {
                let style = Style::default().fg(Color::Black).bg(Color::Yellow);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
            }

            Tag::Small => {
                let style = Style::default().fg(Color::DarkGray);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
            }

            Tag::Var => {
                let style = Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
            }

            // ── Keyboard key ──────────────────────────────────────────────────
            Tag::Kbd => {
                let style = Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD);
                self.add_span("[".to_string(), style, None);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.pop_style();
                self.add_span("]".to_string(), style, None);
            }

            // ── Subscript / Superscript ───────────────────────────────────────
            Tag::Sub => {
                self.add_span("_".to_string(), self.current_style(), None);
                for child in &el.children { self.walk(child); }
            }

            Tag::Sup => {
                self.add_span("^".to_string(), self.current_style(), None);
                for child in &el.children { self.walk(child); }
            }

            // ── Abbreviation ──────────────────────────────────────────────────
            Tag::Abbr => {
                for child in &el.children { self.walk(child); }
                if let Some(title) = el.attrs.get("title") {
                    let style = Style::default().fg(Color::DarkGray);
                    self.add_span(format!(" ({})", title), style, None);
                }
            }

            // ── Quotation ─────────────────────────────────────────────────────
            Tag::Q => {
                self.add_span("\u{201C}".to_string(), self.current_style(), None);
                for child in &el.children { self.walk(child); }
                self.add_span("\u{201D}".to_string(), self.current_style(), None);
            }

            // ── Figure / Figcaption ───────────────────────────────────────────
            Tag::Figcaption => {
                self.flush_inline();
                let style = Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC);
                self.push_style(style, None);
                self.add_span("  ↳ ".to_string(), style, None);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.pop_style();
            }

            // ── Details / Summary ─────────────────────────────────────────────
            Tag::Details => {
                self.flush_inline();
                self.push_blank_lines(1);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.push_blank_lines(1);
            }

            Tag::Summary => {
                self.flush_inline();
                let style = Style::default().add_modifier(Modifier::BOLD);
                self.add_span("▶ ".to_string(), style, None);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                self.flush_inline();
                self.pop_style();
            }

            // ── Time ─────────────────────────────────────────────────────────
            Tag::Time => {
                let style = Style::default().add_modifier(Modifier::ITALIC);
                self.push_style(style, None);
                for child in &el.children { self.walk(child); }
                // Append datetime attr if different from displayed text
                if let Some(dt) = el.attrs.get("datetime") {
                    let displayed: String = self.inline_spans.iter().map(|s| s.text.as_str()).collect();
                    if !displayed.trim().contains(dt.as_str()) {
                        let dim = Style::default().fg(Color::DarkGray);
                        self.add_span(format!(" ({})", dt), dim, None);
                    }
                }
                self.pop_style();
            }

            // ── Span and unknown inlines ──────────────────────────────────────
            Tag::Span | Tag::Unknown(_) => {
                for child in &el.children { self.walk(child); }
            }

            Tag::Img => {} // handled in walk() as DomNode::Image

            // ── Remaining block containers ────────────────────────────────────
            _ => {
                if el.tag.is_block() { self.flush_inline(); }
                for child in &el.children { self.walk(child); }
                if el.tag.is_block() { self.flush_inline(); }
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Truncate a string to at most `max_width` terminal columns.
fn truncate_to_width(s: &str, max_width: usize) -> String {
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);
        if w + cw > max_width { break; }
        out.push(c);
        w += cw;
    }
    out
}

/// Truncate a URL for display purposes.
fn truncate_url(url: &str, max: usize) -> &str {
    if url.len() <= max { url }
    else { &url[..max] }
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

    #[test]
    fn test_render_del_mark() {
        let page = render_html("<html><body><del>old</del> <mark>new</mark></body></html>");
        let all_spans: Vec<_> = page.lines.iter().flat_map(|l| l.spans.iter()).collect();
        let del_span = all_spans.iter().find(|s| s.text.contains("old")).unwrap();
        assert!(del_span.style.add_modifier.contains(Modifier::CROSSED_OUT));
        let mark_span = all_spans.iter().find(|s| s.text.contains("new")).unwrap();
        assert_eq!(mark_span.style.bg, Some(Color::Yellow));
    }

    #[test]
    fn test_render_kbd() {
        let page = render_html("<html><body><kbd>Ctrl+C</kbd></body></html>");
        let text: String = page.lines.iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.text.as_str())
            .collect();
        assert!(text.contains('[') && text.contains("Ctrl+C") && text.contains(']'));
    }

    #[test]
    fn test_link_col_range_set() {
        let page = render_html(r#"<html><body><a href="/x">Click here</a></body></html>"#);
        assert!(!page.links.is_empty());
        let link = &page.links[0];
        // col_range should not be 0..0 (it was set during flush_inline)
        assert!(link.col_range.end > 0, "col_range should be set, got {:?}", link.col_range);
    }
}
