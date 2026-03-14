use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use futures::StreamExt;
use tokio::sync::mpsc;
use url::Url;

use crate::browser::{BrowserState, LoadState};
use crate::fetcher::{self, FetchBody, FetchResult};
use crate::image::ChafaRenderer;
use crate::keybindings::{self, Action};
use crate::parser;
use crate::renderer;

// ── App mode ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    AddressBar,
    Search,
}

// ── App events ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum AppEvent {
    TermKey(KeyEvent),
    TermMouse(MouseEvent),
    TermResize(u16, u16),
    Tick,
    FetchComplete { tab_id: usize, result: FetchResult },
    FetchError { tab_id: usize, error: String },
    /// Async image render completed. `image_id` uniquely identifies the
    /// placeholder within the page; `lines` are the parsed styled rows.
    ImageRendered {
        image_id: usize,
        tab_id: usize,
        lines: Vec<crate::browser::StyledLine>,
    },
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub browser: BrowserState,
    pub mode: AppMode,
    pub input_buffer: String,
    pub cursor_pos: usize,
    pub last_key: Option<(KeyCode, Instant)>,
    pub http_client: Arc<reqwest::Client>,
    pub chafa: Arc<ChafaRenderer>,
    pub event_tx: mpsc::UnboundedSender<AppEvent>,
    pub event_rx: mpsc::UnboundedReceiver<AppEvent>,
    pub spinner_tick: usize,
    pub is_dirty: bool,
    pub should_quit: bool,
    pub viewport_width: u16,
    pub viewport_height: u16,
    /// Optional URL to navigate to on first draw (from CLI argument).
    initial_url: Option<Url>,
}

impl App {
    pub fn new(initial_url: Option<Url>) -> Result<Self> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let http_client = Arc::new(fetcher::build_client()?);
        let chafa = Arc::new(ChafaRenderer::new());
        Ok(Self {
            browser: BrowserState::new(),
            mode: AppMode::Normal,
            input_buffer: String::new(),
            cursor_pos: 0,
            last_key: None,
            http_client,
            chafa,
            event_tx,
            event_rx,
            spinner_tick: 0,
            is_dirty: true,
            should_quit: false,
            viewport_width: 80,
            viewport_height: 24,
            initial_url,
        })
    }

    // ── Main event loop ───────────────────────────────────────────────────────

    pub async fn run<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
    ) -> Result<()> {
        // If a URL was passed on the command line, navigate immediately
        if let Some(url) = self.initial_url.take() {
            self.navigate_to(url);
        }

        let mut term_events = crossterm::event::EventStream::new();
        let mut tick_interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                Some(Ok(event)) = term_events.next() => {
                    self.handle_crossterm_event(event);
                }
                Some(app_event) = self.event_rx.recv() => {
                    self.handle_app_event(app_event);
                }
                _ = tick_interval.tick() => {
                    self.spinner_tick = self.spinner_tick.wrapping_add(1);
                    let tab = self.browser.current_tab();
                    if matches!(tab.load_state, LoadState::Loading { .. }) {
                        self.is_dirty = true;
                    }
                }
            }

            if self.is_dirty {
                terminal.draw(|frame| {
                    let area = frame.area();
                    self.viewport_width = area.width;
                    self.viewport_height = area.height;
                    let content_h = area.height.saturating_sub(3);
                    self.browser.current_tab_mut().scroll.viewport_height = content_h as usize;
                    crate::ui::draw(frame, self);
                })?;
                self.is_dirty = false;
            }

            if self.should_quit { break; }
        }
        Ok(())
    }

    fn handle_crossterm_event(&mut self, event: crossterm::event::Event) {
        use crossterm::event::Event;
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => {
                self.handle_app_event(AppEvent::TermMouse(mouse));
            }
            Event::Resize(w, h) => {
                self.viewport_width = w;
                self.viewport_height = h;
                // Re-render current page at new width for text reflow
                self.rerender_page(w);
                self.is_dirty = true;
            }
            _ => {}
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        let action = match self.mode {
            AppMode::Normal => {
                // Handle gg double-key
                if key.code == KeyCode::Char('g') {
                    if let Some((KeyCode::Char('g'), t)) = self.last_key {
                        if t.elapsed() < Duration::from_millis(500) {
                            self.last_key = None;
                            self.is_dirty = true;
                            self.apply_action(Action::ScrollTop);
                            return;
                        }
                    }
                    self.last_key = Some((KeyCode::Char('g'), Instant::now()));
                    return;
                }
                self.last_key = None;
                keybindings::map_normal(key)
            }
            AppMode::AddressBar | AppMode::Search => keybindings::map_input(key),
        };

        if let Some(action) = action {
            self.is_dirty = true;
            self.apply_action(action);
        }
    }

    fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::FetchComplete { tab_id, result } => {
                self.on_fetch_complete(tab_id, result);
                self.is_dirty = true;
            }
            AppEvent::FetchError { tab_id, error } => {
                if let Some(tab) = self.browser.tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.load_state = LoadState::Error(error);
                }
                self.is_dirty = true;
            }
            AppEvent::ImageRendered { image_id, tab_id, lines: rendered } => {
                if let Some(tab) = self.browser.tabs.iter_mut().find(|t| t.id == tab_id) {
                    if let Some(page) = &mut tab.page {
                        // Find the placeholder by image_id (robust to prior splices)
                        if let Some(ph_idx) = page.lines.iter().position(|l| {
                            matches!(&l.line_type,
                                crate::browser::LineType::ImagePlaceholder { image_id: id, .. }
                                if *id == image_id)
                        }) {
                            let n_new = rendered.len();
                            // Replace the single placeholder with N rendered rows
                            page.lines.splice(ph_idx..ph_idx + 1, rendered);

                            // Adjust link line_indices for the inserted rows
                            let delta = n_new as isize - 1;
                            if delta != 0 {
                                for link in &mut page.links {
                                    if link.line_idx > ph_idx {
                                        link.line_idx =
                                            (link.line_idx as isize + delta).max(0) as usize;
                                    }
                                }
                            }

                            tab.scroll.total_lines = page.lines.len();
                        }
                    }
                }
                self.is_dirty = true;
            }
            AppEvent::TermMouse(mouse) => {
                use crossterm::event::MouseEventKind;
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        self.browser.current_tab_mut().scroll.scroll_down(3);
                        self.is_dirty = true;
                    }
                    MouseEventKind::ScrollUp => {
                        self.browser.current_tab_mut().scroll.scroll_up(3);
                        self.is_dirty = true;
                    }
                    _ => {}
                }
            }
            AppEvent::TermResize(w, h) => {
                self.viewport_width = w;
                self.viewport_height = h;
                self.is_dirty = true;
            }
            AppEvent::TermKey(_) | AppEvent::Tick => {}
        }
    }

    // ── Action handler ────────────────────────────────────────────────────────

    fn apply_action(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,

            Action::ScrollDown(n) => self.browser.current_tab_mut().scroll.scroll_down(n),
            Action::ScrollUp(n) => self.browser.current_tab_mut().scroll.scroll_up(n),
            Action::ScrollTop => self.browser.current_tab_mut().scroll.scroll_top(),
            Action::ScrollBottom => self.browser.current_tab_mut().scroll.scroll_bottom(),
            Action::ScrollPageDown => self.browser.current_tab_mut().scroll.page_down(),
            Action::ScrollPageUp => self.browser.current_tab_mut().scroll.page_up(),

            Action::FocusNextLink => self.focus_link(1),
            Action::FocusPrevLink => self.focus_link(-1),
            Action::FollowLink => self.follow_focused_link(),

            Action::GoBack => {
                if let Some(url) = self.browser.current_tab_mut().go_back() {
                    self.navigate_to(url);
                }
            }
            Action::GoForward => {
                if let Some(url) = self.browser.current_tab_mut().go_forward() {
                    self.navigate_to(url);
                }
            }
            Action::Reload => {
                if let Some(url) = self.browser.current_tab().url.clone() {
                    self.navigate_to(url);
                }
            }

            Action::EnterAddressBar => {
                self.mode = AppMode::AddressBar;
                let current = self.browser.current_tab()
                    .url.as_ref().map(|u| u.to_string()).unwrap_or_default();
                self.input_buffer = current;
                self.cursor_pos = self.input_buffer.len();
            }
            Action::ExitMode => {
                self.mode = AppMode::Normal;
                self.input_buffer.clear();
                self.cursor_pos = 0;
            }
            Action::SubmitInput => {
                match self.mode {
                    AppMode::AddressBar => {
                        let raw = self.input_buffer.trim().to_string();
                        self.mode = AppMode::Normal;
                        self.input_buffer.clear();
                        self.cursor_pos = 0;
                        if !raw.is_empty() {
                            match normalize_url(&raw) {
                                Ok(u) => self.navigate_to(u),
                                Err(e) => {
                                    self.browser.current_tab_mut().load_state =
                                        LoadState::Error(format!("Invalid URL: {}", e));
                                }
                            }
                        }
                    }
                    AppMode::Search => {
                        // TODO: implement search
                        self.mode = AppMode::Normal;
                        self.input_buffer.clear();
                        self.cursor_pos = 0;
                    }
                    _ => {}
                }
            }

            Action::SearchStart => {
                self.mode = AppMode::Search;
                self.input_buffer.clear();
                self.cursor_pos = 0;
            }
            Action::SearchNext | Action::SearchPrev => {
                // TODO: implement search navigation
            }

            // Input editing
            Action::InputChar(c) => {
                self.input_buffer.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
            }
            Action::InputBackspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.input_buffer.remove(self.cursor_pos);
                }
            }
            Action::InputDelete => {
                if self.cursor_pos < self.input_buffer.len() {
                    self.input_buffer.remove(self.cursor_pos);
                }
            }
            Action::InputLeft => {
                if self.cursor_pos > 0 { self.cursor_pos -= 1; }
            }
            Action::InputRight => {
                if self.cursor_pos < self.input_buffer.len() { self.cursor_pos += 1; }
            }
            Action::InputHome => self.cursor_pos = 0,
            Action::InputEnd => self.cursor_pos = self.input_buffer.len(),

            // Tabs
            Action::NewTab => { self.browser.new_tab(); }
            Action::CloseTab => { self.browser.close_tab(); }
            Action::NextTab => self.browser.next_tab(),
            Action::PrevTab => self.browser.prev_tab(),
            Action::SwitchTab(i) => self.browser.switch_to(i),
        }
    }

    // ── Navigation helpers ────────────────────────────────────────────────────

    fn navigate_to(&mut self, url: Url) {
        let tab_id = self.browser.current_tab().id;
        let tab = self.browser.current_tab_mut();
        tab.load_state = LoadState::Loading { url: url.clone() };
        tab.page = None;
        tab.scroll = Default::default();

        let tx = self.event_tx.clone();
        let client = Arc::clone(&self.http_client);

        tokio::spawn(async move {
            match fetcher::fetch(&client, url).await {
                Ok(result) => { let _ = tx.send(AppEvent::FetchComplete { tab_id, result }); }
                Err(e) => { let _ = tx.send(AppEvent::FetchError { tab_id, error: e.to_string() }); }
            }
        });
    }

    fn on_fetch_complete(&mut self, tab_id: usize, result: FetchResult) {
        let Some(tab) = self.browser.tabs.iter_mut().find(|t| t.id == tab_id) else {
            return;
        };

        let final_url = result.url.clone();
        let is_history_nav = tab.is_history_nav;
        tab.is_history_nav = false;

        match result.body {
            FetchBody::Html(html) => {
                let base_url = final_url.clone();
                let col_width = self.viewport_width.max(40);

                let parsed = parser::parse(&html, &base_url);
                let title = parsed.title.clone();

                let mut page = renderer::render(&parsed.root, &base_url, col_width);
                page.title = title.clone();
                page.url = final_url.clone();

                let total = page.lines.len();

                // Store DOM for reflow on resize
                let tab = self.browser.tabs.iter_mut().find(|t| t.id == tab_id).unwrap();
                tab.dom = Some((parsed.root, base_url.clone()));
                tab.page = Some(page);
                tab.title = title.clone();
                tab.url = Some(final_url.clone());
                tab.load_state = LoadState::Idle;
                tab.scroll.total_lines = total;
                tab.scroll.offset = 0;

                if !is_history_nav {
                    tab.push_history(final_url, title);
                }

                // Spawn image render tasks
                self.spawn_image_tasks(tab_id);
            }
            FetchBody::Binary { mime, .. } => {
                let tab = self.browser.tabs.iter_mut().find(|t| t.id == tab_id).unwrap();
                tab.load_state = LoadState::Error(
                    format!("Binary content ({}); cannot display in browser", mime)
                );
                tab.url = Some(final_url);
            }
        }
    }

    /// Re-render the current page at a new terminal width (on resize).
    fn rerender_page(&mut self, new_width: u16) {
        let tab = self.browser.current_tab_mut();
        if let Some((dom, base_url)) = &tab.dom {
            let col_width = new_width.max(40);
            let mut page = renderer::render(dom, base_url, col_width);
            if let Some(existing) = &tab.page {
                page.title = existing.title.clone();
                page.focused_link = existing.focused_link;
            }
            let total = page.lines.len();
            tab.scroll.total_lines = total;
            // Clamp scroll offset to new total
            if tab.scroll.offset >= total {
                tab.scroll.offset = total.saturating_sub(1);
            }
            tab.page = Some(page);
        }
    }

    fn spawn_image_tasks(&mut self, tab_id: usize) {
        let Some(tab) = self.browser.tabs.iter().find(|t| t.id == tab_id) else {
            return;
        };
        let Some(page) = &tab.page else { return; };

        // Collect (image_id, src_url) for every placeholder on this page.
        let image_tasks: Vec<(usize, String)> = page.lines.iter()
            .filter_map(|line| {
                if let crate::browser::LineType::ImagePlaceholder { image_id, src, .. } = &line.line_type {
                    if !src.is_empty() { Some((*image_id, src.clone())) } else { None }
                } else {
                    None
                }
            })
            .collect();

        let col_width = self.viewport_width.max(40);
        // Image height: half the column width (typical terminal cell is ~2:1 tall:wide),
        // giving a roughly square image. Capped at 40 rows to avoid flooding the page.
        let img_height: u16 = (col_width / 2).min(40).max(10);

        for (image_id, src_url) in image_tasks {
            let Ok(url) = Url::parse(&src_url) else { continue; };
            let tx = self.event_tx.clone();
            let client = Arc::clone(&self.http_client);
            let chafa = Arc::clone(&self.chafa);

            tokio::spawn(async move {
                let bytes = match fetcher::fetch_bytes(&client, url).await {
                    Ok(b) => b,
                    Err(_) => return,
                };
                match chafa.render_image(&bytes, col_width, img_height) {
                    Ok(lines) if !lines.is_empty() => {
                        let _ = tx.send(AppEvent::ImageRendered { image_id, tab_id, lines });
                    }
                    _ => {} // silently ignore render failures
                }
            });
        }
    }

    fn focus_link(&mut self, direction: i32) {
        let tab = self.browser.current_tab_mut();
        let Some(page) = &mut tab.page else { return; };
        if page.links.is_empty() { return; }

        let current = page.focused_link.unwrap_or(0);
        let count = page.links.len() as i32;
        let next = ((current as i32 + direction).rem_euclid(count)) as usize;
        page.focused_link = Some(next);

        let link_line = page.links[next].line_idx;
        if link_line < tab.scroll.offset {
            tab.scroll.offset = link_line;
        } else if link_line >= tab.scroll.offset + tab.scroll.viewport_height {
            tab.scroll.offset = link_line.saturating_sub(tab.scroll.viewport_height / 2);
        }
    }

    fn follow_focused_link(&mut self) {
        let href = {
            let tab = self.browser.current_tab();
            let Some(page) = &tab.page else { return; };
            let Some(idx) = page.focused_link else { return; };
            page.links.get(idx).map(|l| l.href.clone())
        };

        if let Some(href) = href {
            match Url::parse(&href) {
                Ok(url) => self.navigate_to(url),
                Err(e) => {
                    self.browser.current_tab_mut().load_state =
                        LoadState::Error(format!("Invalid URL: {}", e));
                }
            }
        }
    }
}

fn normalize_url(raw: &str) -> anyhow::Result<Url> {
    if let Ok(u) = Url::parse(raw) {
        if u.scheme() == "http" || u.scheme() == "https" || u.scheme() == "file" {
            return Ok(u);
        }
    }
    let with_scheme = format!("https://{}", raw);
    Ok(Url::parse(&with_scheme)?)
}
