use unicode_width::UnicodeWidthStr;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

use crate::app::AppMode;

pub struct AddressBar<'a> {
    pub url: &'a str,
    pub input: &'a str,
    pub cursor_pos: usize,
    pub mode: &'a AppMode,
}

impl<'a> Widget for AddressBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width < 5 { return; }

        let y = area.y;
        let total_w = area.width as usize;

        match self.mode {
            AppMode::Normal => {
                let prefix = " ↗ ";
                let prefix_style = Style::default().fg(Color::DarkGray);
                let url_style = Style::default().fg(Color::Gray);

                // Draw prefix
                let mut x = area.x;
                for c in prefix.chars() {
                    if x >= area.x + area.width { break; }
                    buf[(x, y)].set_char(c).set_style(prefix_style);
                    x += 1;
                }

                // Truncate URL to fit
                let avail = total_w.saturating_sub(prefix.len());
                let display = truncate_url(self.url, avail);
                for c in display.chars() {
                    if x >= area.x + area.width { break; }
                    buf[(x, y)].set_char(c).set_style(url_style);
                    x += 1;
                }
            }

            AppMode::AddressBar => {
                let prefix = " URL: ";
                let prefix_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
                let input_style = Style::default().fg(Color::White);
                let cursor_style = Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD);

                let mut x = area.x;
                for c in prefix.chars() {
                    if x >= area.x + area.width { break; }
                    buf[(x, y)].set_char(c).set_style(prefix_style);
                    x += 1;
                }

                let avail = total_w.saturating_sub(prefix.len());
                let (view, visual_cursor) = sliding_window(self.input, self.cursor_pos, avail);

                for (i, c) in view.chars().enumerate() {
                    if x >= area.x + area.width { break; }
                    let style = if i == visual_cursor { cursor_style } else { input_style };
                    buf[(x, y)].set_char(c).set_style(style);
                    x += 1;
                }
                // Block cursor at end
                if visual_cursor >= view.chars().count() && x < area.x + area.width {
                    buf[(x, y)].set_char(' ').set_style(cursor_style);
                }
            }

            AppMode::Search => {
                let prefix = " / ";
                let prefix_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
                let input_style = Style::default().fg(Color::White);
                let cursor_style = Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan);

                let mut x = area.x;
                for c in prefix.chars() {
                    if x >= area.x + area.width { break; }
                    buf[(x, y)].set_char(c).set_style(prefix_style);
                    x += 1;
                }

                let avail = total_w.saturating_sub(prefix.len());
                let (view, visual_cursor) = sliding_window(self.input, self.cursor_pos, avail);

                for (i, c) in view.chars().enumerate() {
                    if x >= area.x + area.width { break; }
                    let style = if i == visual_cursor { cursor_style } else { input_style };
                    buf[(x, y)].set_char(c).set_style(style);
                    x += 1;
                }
                if visual_cursor >= view.chars().count() && x < area.x + area.width {
                    buf[(x, y)].set_char(' ').set_style(cursor_style);
                }
            }
        }
    }
}

/// Truncate a URL to fit `max_cols` terminal columns, with `…` if needed.
fn truncate_url(url: &str, max_cols: usize) -> String {
    if max_cols == 0 { return String::new(); }
    let w = UnicodeWidthStr::width(url);
    if w <= max_cols {
        url.to_string()
    } else {
        let mut out = String::new();
        let mut col = 0usize;
        for c in url.chars() {
            let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);
            if col + cw + 1 > max_cols { break; } // leave room for …
            out.push(c);
            col += cw;
        }
        out.push('…');
        out
    }
}

/// Return a view of `input` that shows `cursor_pos` within `width` columns,
/// plus the visual cursor position within that view.
fn sliding_window(input: &str, cursor_pos: usize, width: usize) -> (String, usize) {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();

    if len == 0 { return (String::new(), 0); }

    // If everything fits, no sliding needed
    let total_w = UnicodeWidthStr::width(input);
    if total_w < width {
        return (input.to_string(), cursor_pos.min(len));
    }

    // Slide window so cursor is near the right edge
    let cursor = cursor_pos.min(len);
    let window_end = cursor + 1;
    let window_start = window_end.saturating_sub(width.saturating_sub(1));

    let view: String = chars[window_start..window_end.min(len)].iter().collect();
    let visual = cursor - window_start;
    (view, visual)
}
