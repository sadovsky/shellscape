use std::collections::HashMap;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use ratatui::style::Color;
use url::Url;

// ── Public IR types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum DomNode {
    Document(Vec<DomNode>),
    Element(Element),
    Text(String),
    Image(ImageNode),
}

#[derive(Debug, Clone)]
pub struct Element {
    pub tag: Tag,
    pub attrs: AttrMap,
    pub children: Vec<DomNode>,
    pub style: ComputedStyle,
}

#[derive(Debug, Clone)]
pub struct ImageNode {
    /// Absolute URL (resolved against base_url during parse)
    pub src: String,
    pub alt: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

pub type AttrMap = HashMap<String, String>;

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Html, Head, Body,
    H1, H2, H3, H4, H5, H6,
    P, Div, Span, Section, Article, Main, Nav, Aside, Header, Footer,
    A, Strong, Em, B, I, Code, Pre, Blockquote,
    Ul, Ol, Li,
    Dl, Dt, Dd,
    Table, Caption, Tr, Td, Th, THead, TBody, TFoot,
    Img, Br, Hr,
    Del, S, Ins, U,
    Mark, Kbd, Sub, Sup, Abbr,
    Small, Q, Cite, Address, Samp, Var,
    Figure, Figcaption,
    Details, Summary,
    Time,
    Script, Style, Noscript,
    // Ignored layout hints
    Colgroup, Col,
    Unknown(String),
}

impl Tag {
    pub fn from_str(s: &str) -> Self {
        match s {
            "html" => Tag::Html, "head" => Tag::Head, "body" => Tag::Body,
            "h1" => Tag::H1, "h2" => Tag::H2, "h3" => Tag::H3,
            "h4" => Tag::H4, "h5" => Tag::H5, "h6" => Tag::H6,
            "p" => Tag::P, "div" => Tag::Div, "span" => Tag::Span,
            "section" => Tag::Section, "article" => Tag::Article,
            "main" => Tag::Main, "nav" => Tag::Nav, "aside" => Tag::Aside,
            "header" => Tag::Header, "footer" => Tag::Footer,
            "a" => Tag::A, "strong" => Tag::Strong, "em" => Tag::Em,
            "b" => Tag::B, "i" => Tag::I,
            "code" => Tag::Code, "pre" => Tag::Pre, "blockquote" => Tag::Blockquote,
            "ul" => Tag::Ul, "ol" => Tag::Ol, "li" => Tag::Li,
            "dl" => Tag::Dl, "dt" => Tag::Dt, "dd" => Tag::Dd,
            "table" => Tag::Table, "caption" => Tag::Caption,
            "tr" => Tag::Tr, "td" => Tag::Td, "th" => Tag::Th,
            "thead" => Tag::THead, "tbody" => Tag::TBody, "tfoot" => Tag::TFoot,
            "img" => Tag::Img, "br" => Tag::Br, "hr" => Tag::Hr,
            "del" => Tag::Del, "s" => Tag::S, "ins" => Tag::Ins, "u" => Tag::U,
            "mark" => Tag::Mark, "kbd" => Tag::Kbd,
            "sub" => Tag::Sub, "sup" => Tag::Sup, "abbr" => Tag::Abbr,
            "small" => Tag::Small, "q" => Tag::Q, "cite" => Tag::Cite,
            "address" => Tag::Address, "samp" => Tag::Samp, "var" => Tag::Var,
            "figure" => Tag::Figure, "figcaption" => Tag::Figcaption,
            "details" => Tag::Details, "summary" => Tag::Summary,
            "time" => Tag::Time,
            "script" => Tag::Script, "style" => Tag::Style, "noscript" => Tag::Noscript,
            "colgroup" => Tag::Colgroup, "col" => Tag::Col,
            other => Tag::Unknown(other.to_string()),
        }
    }

    pub fn is_block(&self) -> bool {
        matches!(self,
            Tag::Html | Tag::Head | Tag::Body |
            Tag::H1 | Tag::H2 | Tag::H3 | Tag::H4 | Tag::H5 | Tag::H6 |
            Tag::P | Tag::Div | Tag::Section | Tag::Article | Tag::Main |
            Tag::Nav | Tag::Aside | Tag::Header | Tag::Footer |
            Tag::Pre | Tag::Blockquote | Tag::Ul | Tag::Ol | Tag::Li |
            Tag::Dl | Tag::Dt | Tag::Dd |
            Tag::Table | Tag::Caption | Tag::Tr | Tag::THead | Tag::TBody | Tag::TFoot |
            Tag::Hr | Tag::Br | Tag::Address | Tag::Figure | Tag::Figcaption |
            Tag::Details
        )
    }

    pub fn is_stripped(&self) -> bool {
        matches!(self, Tag::Script | Tag::Style | Tag::Noscript | Tag::Colgroup | Tag::Col)
    }
}

// ── Computed style ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ComputedStyle {
    pub color: Option<Color>,
    pub bg_color: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub margin_top: u16,
    pub margin_bottom: u16,
}

impl ComputedStyle {
    pub fn for_tag(tag: &Tag) -> Self {
        match tag {
            Tag::H1 => ComputedStyle { bold: true, margin_top: 1, margin_bottom: 1,
                color: Some(Color::Magenta), ..Default::default() },
            Tag::H2 => ComputedStyle { bold: true, margin_top: 1, margin_bottom: 1,
                color: Some(Color::Cyan), ..Default::default() },
            Tag::H3 => ComputedStyle { bold: true, margin_top: 1, margin_bottom: 0,
                color: Some(Color::Yellow), ..Default::default() },
            Tag::H4 | Tag::H5 | Tag::H6 => ComputedStyle {
                bold: true, ..Default::default()
            },
            Tag::Strong | Tag::B => ComputedStyle { bold: true, ..Default::default() },
            Tag::Em | Tag::I | Tag::Cite | Tag::Var => ComputedStyle { italic: true, ..Default::default() },
            Tag::Code | Tag::Samp => ComputedStyle { color: Some(Color::Green), ..Default::default() },
            Tag::Pre => ComputedStyle { color: Some(Color::Green), margin_top: 1,
                margin_bottom: 1, ..Default::default() },
            Tag::Blockquote => ComputedStyle { color: Some(Color::DarkGray),
                italic: true, margin_top: 1, margin_bottom: 1, ..Default::default() },
            Tag::A => ComputedStyle { color: Some(Color::Blue),
                underline: true, ..Default::default() },
            Tag::P => ComputedStyle { margin_bottom: 1, ..Default::default() },
            Tag::Del | Tag::S => ComputedStyle { strikethrough: true, ..Default::default() },
            Tag::Ins | Tag::U => ComputedStyle { underline: true, ..Default::default() },
            Tag::Mark => ComputedStyle { color: Some(Color::Black),
                bg_color: Some(Color::Yellow), ..Default::default() },
            Tag::Small => ComputedStyle { color: Some(Color::DarkGray), ..Default::default() },
            Tag::Address => ComputedStyle { italic: true, margin_top: 1, margin_bottom: 1,
                ..Default::default() },
            Tag::Figcaption => ComputedStyle { italic: true,
                color: Some(Color::DarkGray), ..Default::default() },
            Tag::Summary => ComputedStyle { bold: true, ..Default::default() },
            Tag::Time => ComputedStyle { italic: true, ..Default::default() },
            _ => ComputedStyle::default(),
        }
    }
}

// ── Parse result ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ParseResult {
    pub root: DomNode,
    pub title: String,
}

// ── Main parser ──────────────────────────────────────────────────────────────

pub fn parse(html: &str, base_url: &Url) -> ParseResult {
    let dom = parse_document(RcDom::default(), Default::default())
        .one(html);

    let mut title = String::from("Untitled");
    let children = walk_children(&dom.document, &mut title, base_url);

    ParseResult {
        root: DomNode::Document(children),
        title,
    }
}

fn walk_children(handle: &Handle, title: &mut String, base_url: &Url) -> Vec<DomNode> {
    handle.children.borrow().iter()
        .filter_map(|child| walk_node(child, title, base_url))
        .collect()
}

fn walk_node(handle: &Handle, title: &mut String, base_url: &Url) -> Option<DomNode> {
    match &handle.data {
        NodeData::Document => {
            let children = walk_children(handle, title, base_url);
            Some(DomNode::Document(children))
        }

        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            let normalized = normalize_whitespace(&text);
            if normalized.is_empty() {
                None
            } else {
                Some(DomNode::Text(normalized))
            }
        }

        NodeData::Element { name, attrs, .. } => {
            let tag_name = name.local.as_ref().to_lowercase();
            let tag = Tag::from_str(&tag_name);

            let attrs_map: AttrMap = attrs.borrow().iter().map(|a| {
                (a.name.local.as_ref().to_string(), a.value.to_string())
            }).collect();

            if tag.is_stripped() {
                return None;
            }

            if tag_name == "title" {
                let text = extract_text(handle);
                if !text.is_empty() {
                    *title = text;
                }
                return None;
            }

            if tag == Tag::Img {
                let raw_src = attrs_map.get("src").cloned().unwrap_or_default();
                let src = resolve_url(base_url, &raw_src).unwrap_or(raw_src);
                let alt = attrs_map.get("alt").cloned().unwrap_or_default();
                let width = attrs_map.get("width").and_then(|w| w.parse::<u32>().ok());
                let height = attrs_map.get("height").and_then(|h| h.parse::<u32>().ok());
                return Some(DomNode::Image(ImageNode { src, alt, width, height }));
            }

            // Resolve href for anchor tags
            let mut resolved_attrs = attrs_map.clone();
            if tag == Tag::A {
                if let Some(href) = attrs_map.get("href") {
                    if let Some(abs) = resolve_url(base_url, href) {
                        resolved_attrs.insert("href".into(), abs);
                    }
                }
            }

            let computed_style = ComputedStyle::for_tag(&tag);
            let children = walk_children(handle, title, base_url);

            Some(DomNode::Element(Element {
                tag,
                attrs: resolved_attrs,
                children,
                style: computed_style,
            }))
        }

        _ => None,
    }
}

fn extract_text(handle: &Handle) -> String {
    let mut out = String::new();
    for child in handle.children.borrow().iter() {
        if let NodeData::Text { contents } = &child.data {
            out.push_str(&contents.borrow());
        } else {
            out.push_str(&extract_text(child));
        }
    }
    out.trim().to_string()
}

fn normalize_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut last_was_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(c);
            last_was_space = false;
        }
    }
    result
}

/// Resolve a potentially-relative URL against a base. Returns None for
/// empty, fragment-only, or javascript: URLs.
pub fn resolve_url(base: &Url, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') || href.starts_with("javascript:") {
        return None;
    }
    match base.join(href) {
        Ok(u) => Some(u.to_string()),
        Err(_) => Some(href.to_string()),
    }
}

/// Parse a CSS color string into a ratatui Color.
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex_color(hex);
    }
    if s.starts_with("rgb(") {
        return parse_rgb_color(s);
    }
    named_color(s)
}

fn parse_hex_color(hex: &str) -> Option<Color> {
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

fn parse_rgb_color(s: &str) -> Option<Color> {
    let inner = s.strip_prefix("rgb(")?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != 3 { return None; }
    let r = parts[0].trim().parse::<u8>().ok()?;
    let g = parts[1].trim().parse::<u8>().ok()?;
    let b = parts[2].trim().parse::<u8>().ok()?;
    Some(Color::Rgb(r, g, b))
}

fn named_color(name: &str) -> Option<Color> {
    match name.to_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" | "darkred" => Some(Color::Red),
        "green" | "darkgreen" => Some(Color::Green),
        "yellow" | "gold" => Some(Color::Yellow),
        "blue" | "darkblue" => Some(Color::Blue),
        "magenta" | "purple" | "darkmagenta" => Some(Color::Magenta),
        "cyan" | "darkcyan" | "teal" => Some(Color::Cyan),
        "white" | "snow" | "ivory" => Some(Color::White),
        "gray" | "grey" | "silver" => Some(Color::Gray),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        "lightred" | "salmon" | "coral" | "tomato" | "orangered" => Some(Color::LightRed),
        "lightgreen" | "lime" | "limegreen" => Some(Color::LightGreen),
        "lightyellow" | "lemonchiffon" => Some(Color::LightYellow),
        "lightblue" | "skyblue" | "deepskyblue" => Some(Color::LightBlue),
        "lightmagenta" | "violet" | "fuchsia" => Some(Color::LightMagenta),
        "lightcyan" | "aqua" => Some(Color::LightCyan),
        "orange" => Some(Color::Rgb(255, 165, 0)),
        "navy" => Some(Color::Rgb(0, 0, 128)),
        "maroon" => Some(Color::Rgb(128, 0, 0)),
        "olive" => Some(Color::Rgb(128, 128, 0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_html() {
        let base = Url::parse("http://example.com").unwrap();
        let result = parse("<html><body><h1>Hello</h1><p>World</p></body></html>", &base);
        assert_eq!(result.title, "Untitled");
        match result.root {
            DomNode::Document(children) => assert!(!children.is_empty()),
            _ => panic!("Expected Document"),
        }
    }

    #[test]
    fn test_parse_title() {
        let base = Url::parse("http://example.com").unwrap();
        let result = parse("<html><head><title>My Page</title></head><body></body></html>", &base);
        assert_eq!(result.title, "My Page");
    }

    #[test]
    fn test_parse_color() {
        assert_eq!(parse_color("#ff0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_color("#f00"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_color("rgb(0, 128, 255)"), Some(Color::Rgb(0, 128, 255)));
        assert_eq!(parse_color("red"), Some(Color::Red));
    }

    #[test]
    fn test_relative_url_resolution() {
        let base = Url::parse("https://example.com/news/").unwrap();
        assert_eq!(
            resolve_url(&base, "/about"),
            Some("https://example.com/about".to_string())
        );
        assert_eq!(
            resolve_url(&base, "article.html"),
            Some("https://example.com/news/article.html".to_string())
        );
        assert_eq!(resolve_url(&base, "#section"), None);
        assert_eq!(resolve_url(&base, "javascript:void(0)"), None);
    }

    #[test]
    fn test_img_src_resolved() {
        let base = Url::parse("https://example.com/").unwrap();
        let result = parse(r#"<html><body><img src="/logo.png" alt="Logo"></body></html>"#, &base);
        fn find_img(nodes: &[DomNode]) -> Option<&ImageNode> {
            for n in nodes {
                match n {
                    DomNode::Image(img) => return Some(img),
                    DomNode::Document(c) | DomNode::Element(crate::parser::Element { children: c, .. }) => {
                        if let Some(img) = find_img(c) { return Some(img); }
                    }
                    _ => {}
                }
            }
            None
        }
        if let DomNode::Document(children) = &result.root {
            let img = find_img(children).expect("should find img");
            assert_eq!(img.src, "https://example.com/logo.png");
            assert_eq!(img.alt, "Logo");
        }
    }
}
