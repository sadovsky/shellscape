use std::ops::Range;
use url::Url;
use ratatui::style::Style;

use crate::parser::DomNode;

// ── Rendered output types (produced by renderer.rs) ─────────────────────────

#[derive(Debug, Clone)]
pub struct StyledSpan {
    pub text: String,
    pub style: Style,
    pub link_idx: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum LineType {
    Normal,
    Heading,
    HorizontalRule,
    ListItem,
    /// A placeholder for an image that may not have been rendered yet.
    /// `image_id` is unique within the page and used to find this line when
    /// the async render task completes — even if prior splices shifted indices.
    ImagePlaceholder {
        image_id: usize,
        src: String,
    },
}

#[derive(Debug, Clone)]
pub struct StyledLine {
    pub spans: Vec<StyledSpan>,
    pub line_type: LineType,
}

impl StyledLine {
    pub fn empty() -> Self {
        Self { spans: vec![], line_type: LineType::Normal }
    }

}

// ── Link registry ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PageLink {
    pub line_idx: usize,
    pub col_range: Range<u16>,
    pub href: String,
    pub text: String,
}

// ── Rendered page ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub url: Url,
    pub title: String,
    pub lines: Vec<StyledLine>,
    pub links: Vec<PageLink>,
    pub focused_link: Option<usize>,
}

// ── Scroll state ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset: usize,
    pub viewport_height: usize,
    pub total_lines: usize,
}

impl ScrollState {
    pub fn scroll_down(&mut self, n: usize) {
        let max = self.total_lines.saturating_sub(self.viewport_height);
        self.offset = (self.offset + n).min(max);
    }
    pub fn scroll_up(&mut self, n: usize) {
        self.offset = self.offset.saturating_sub(n);
    }
    pub fn scroll_top(&mut self) {
        self.offset = 0;
    }
    pub fn scroll_bottom(&mut self) {
        self.offset = self.total_lines.saturating_sub(self.viewport_height);
    }
    pub fn page_down(&mut self) {
        let step = (self.viewport_height / 2).max(1);
        self.scroll_down(step);
    }
    pub fn page_up(&mut self) {
        let step = (self.viewport_height / 2).max(1);
        self.scroll_up(step);
    }
}

// ── Load state ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum LoadState {
    Idle,
    Loading { url: Url },
    Error(String),
}

// ── History ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub url: Url,
}

// ── Tab ───────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Tab {
    pub id: usize,
    pub title: String,
    pub url: Option<Url>,
    /// Parsed DOM tree; stored so the page can be re-rendered on resize.
    pub dom: Option<(DomNode, Url)>,
    /// history[0] = oldest, history[history_idx - 1] = current page.
    /// history_idx is 1-based: 0 means no page yet.
    pub history: Vec<HistoryEntry>,
    pub history_idx: usize,
    pub page: Option<RenderedPage>,
    pub load_state: LoadState,
    pub scroll: ScrollState,
    /// True when the current navigation came from back/forward; prevents
    /// on_fetch_complete from pushing a duplicate history entry.
    pub is_history_nav: bool,
}

impl Tab {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            title: String::from("New Tab"),
            url: None,
            dom: None,
            history: vec![],
            history_idx: 0,
            page: None,
            load_state: LoadState::Idle,
            scroll: ScrollState::default(),
            is_history_nav: false,
        }
    }

    /// Push a new entry and advance the pointer. Truncates any forward history.
    pub fn push_history(&mut self, url: Url) {
        self.history.truncate(self.history_idx);
        self.history.push(HistoryEntry { url });
        self.history_idx = self.history.len();
    }

    /// Returns the URL to navigate to, or None if already at the beginning.
    pub fn go_back(&mut self) -> Option<Url> {
        if self.history_idx > 1 {
            self.history_idx -= 1;
            self.is_history_nav = true;
            Some(self.history[self.history_idx - 1].url.clone())
        } else {
            None
        }
    }

    /// Returns the URL to navigate to, or None if already at the end.
    pub fn go_forward(&mut self) -> Option<Url> {
        if self.history_idx < self.history.len() {
            self.history_idx += 1;
            self.is_history_nav = true;
            Some(self.history[self.history_idx - 1].url.clone())
        } else {
            None
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.history_idx > 1
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_idx < self.history.len()
    }
}

// ── Browser state ─────────────────────────────────────────────────────────────

pub struct BrowserState {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    next_tab_id: usize,
}

impl BrowserState {
    pub fn new() -> Self {
        let first_tab = Tab::new(0);
        Self {
            tabs: vec![first_tab],
            active_tab: 0,
            next_tab_id: 1,
        }
    }

    pub fn current_tab(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }

    pub fn current_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab]
    }

    pub fn new_tab(&mut self) -> usize {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(Tab::new(id));
        self.active_tab = self.tabs.len() - 1;
        id
    }

    pub fn close_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.tabs.remove(self.active_tab);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
        }
    }

    pub fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % self.tabs.len();
    }

    pub fn prev_tab(&mut self) {
        if self.active_tab == 0 {
            self.active_tab = self.tabs.len() - 1;
        } else {
            self.active_tab -= 1;
        }
    }

    pub fn switch_to(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active_tab = idx;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_url(s: &str) -> Url { Url::parse(s).unwrap() }

    #[test]
    fn test_history_push_and_back() {
        let mut tab = Tab::new(0);
        tab.push_history(make_url("http://a.com"));
        tab.push_history(make_url("http://b.com"));
        tab.push_history(make_url("http://c.com"));

        assert_eq!(tab.history_idx, 3);
        assert_eq!(tab.go_back().unwrap().as_str(), "http://b.com/");
        assert_eq!(tab.history_idx, 2);
        assert_eq!(tab.go_back().unwrap().as_str(), "http://a.com/");
        assert_eq!(tab.history_idx, 1);
        assert!(tab.go_back().is_none());
    }

    #[test]
    fn test_history_forward() {
        let mut tab = Tab::new(0);
        tab.push_history(make_url("http://a.com"));
        tab.push_history(make_url("http://b.com"));
        tab.go_back();
        assert_eq!(tab.go_forward().unwrap().as_str(), "http://b.com/");
        assert!(tab.go_forward().is_none());
    }

    #[test]
    fn test_history_truncates_forward_on_new_nav() {
        let mut tab = Tab::new(0);
        tab.push_history(make_url("http://a.com"));
        tab.push_history(make_url("http://b.com"));
        tab.push_history(make_url("http://c.com"));
        tab.go_back(); // now at B
        tab.go_back(); // now at A
        tab.push_history(make_url("http://d.com"));
        // C and B should be gone
        assert_eq!(tab.history.len(), 2);
        assert_eq!(tab.history[1].url.as_str(), "http://d.com/");
        assert!(tab.go_forward().is_none());
    }
}
