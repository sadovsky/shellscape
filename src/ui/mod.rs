pub mod address_bar;
pub mod content;
pub mod status_bar;
pub mod tabs;

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
};

use crate::app::{App, AppMode};

use address_bar::AddressBar;
use content::ContentArea;
use status_bar::StatusBar;
use tabs::TabBar;

pub fn draw(frame: &mut Frame, app: &App) {
    let browser = &app.browser;
    let tab = browser.current_tab();
    let show_tabs = browser.tabs.len() > 1;

    let chunks = Layout::vertical([
        Constraint::Length(if show_tabs { 1 } else { 0 }),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .split(frame.area());

    // Tab bar
    if show_tabs {
        let titles: Vec<String> = browser.tabs.iter().map(|t| t.title.clone()).collect();
        frame.render_widget(
            TabBar { tab_titles: &titles, active: browser.active_tab },
            chunks[0],
        );
    }

    // Address bar
    let current_url = tab.url.as_ref().map(|u| u.as_str()).unwrap_or("");
    frame.render_widget(
        AddressBar {
            url: current_url,
            input: &app.input_buffer,
            cursor_pos: app.cursor_pos,
            editing: matches!(app.mode, AppMode::AddressBar | AppMode::Search),
        },
        chunks[1],
    );

    // Content
    let scroll_offset = tab.scroll.offset;
    let focused_link = tab.page.as_ref().and_then(|p| p.focused_link);
    frame.render_widget(
        ContentArea {
            page: tab.page.as_ref(),
            scroll_offset,
            focused_link,
        },
        chunks[2],
    );

    // Status bar
    let hint = match app.mode {
        AppMode::Normal => " o:URL  H/L:back/fwd  t:tab  /:search  q:quit",
        AppMode::AddressBar => " Enter:go  Esc:cancel",
        AppMode::Search => " Enter:find  Esc:cancel  n/N:next/prev",
    };
    frame.render_widget(
        StatusBar {
            load_state: &tab.load_state,
            spinner_tick: app.spinner_tick,
            mode_hint: hint,
        },
        chunks[3],
    );
}
