use std::ops::Range;
use url::Url;
use ratatui::style::Style;

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
    Heading(u8),
    HorizontalRule,
    CodeBlock,
    BlockQuote,
    ListItem(u8),
    ImagePlaceholder { chafa_output: Option<String>, alt: String },
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
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            spans: vec![StyledSpan { text: text.into(), style: Style::default(), link_idx: None }],
            line_type: LineType::Normal,
        }
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
    pub title: String,
}

// ── Tab ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Tab {
    pub id: usize,
    pub title: String,
    pub url: Option<Url>,
    pub history: Vec<HistoryEntry>,
    pub history_idx: usize,
    pub page: Option<RenderedPage>,
    pub load_state: LoadState,
    pub scroll: ScrollState,
}

impl Tab {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            title: String::from("New Tab"),
            url: None,
            history: vec![],
            history_idx: 0,
            page: None,
            load_state: LoadState::Idle,
            scroll: ScrollState::default(),
        }
    }

    pub fn push_history(&mut self, url: Url, title: String) {
        // Truncate forward history on new navigation
        if self.history_idx < self.history.len() {
            self.history.truncate(self.history_idx);
        }
        self.history.push(HistoryEntry { url, title });
        self.history_idx = self.history.len();
    }

    pub fn go_back(&mut self) -> Option<Url> {
        if self.history_idx > 1 {
            self.history_idx -= 1;
            Some(self.history[self.history_idx - 1].url.clone())
        } else {
            None
        }
    }

    pub fn go_forward(&mut self) -> Option<Url> {
        if self.history_idx < self.history.len() {
            self.history_idx += 1;
            Some(self.history[self.history_idx - 1].url.clone())
        } else {
            None
        }
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
