use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

use crate::browser::LoadState;

const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub struct StatusBar<'a> {
    pub load_state: &'a LoadState,
    pub spinner_tick: usize,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub mode_hint: &'a str,
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 { return; }

        let bar_style = Style::default().bg(Color::DarkGray).fg(Color::White);

        // Fill background
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_char(' ').set_style(bar_style);
        }

        // Left side: nav arrows + status
        let back_arrow = if self.can_go_back { "◂ " } else { "  " };
        let fwd_arrow  = if self.can_go_forward { " ▸" } else { "  " };
        let nav_style_active = Style::default().bg(Color::DarkGray).fg(Color::Cyan).add_modifier(Modifier::BOLD);
        let nav_style_dim    = Style::default().bg(Color::DarkGray).fg(Color::Black);

        let mut x = area.x;
        for (i, c) in back_arrow.chars().enumerate() {
            buf[(x + i as u16, area.y)].set_char(c)
                .set_style(if self.can_go_back { nav_style_active } else { nav_style_dim });
        }
        x += back_arrow.len() as u16;

        let status = match self.load_state {
            LoadState::Idle => " Ready".to_string(),
            LoadState::Loading { url } => {
                let spinner = SPINNER[self.spinner_tick % SPINNER.len()];
                let url_str = url.as_str();
                let truncated = if url_str.len() > 50 {
                    format!("{}…", &url_str[..49])
                } else {
                    url_str.to_string()
                };
                format!(" {} Loading {}…", spinner, truncated)
            }
            LoadState::Error(e) => {
                let truncated = if e.len() > 60 {
                    format!("{}…", &e[..59])
                } else {
                    e.clone()
                };
                format!(" ✗ {}", truncated)
            }
        };

        let status_style = match self.load_state {
            LoadState::Error(_) => Style::default().bg(Color::Red).fg(Color::White)
                .add_modifier(Modifier::BOLD),
            LoadState::Loading { .. } => Style::default().bg(Color::DarkGray).fg(Color::Yellow),
            _ => bar_style,
        };

        let hint = self.mode_hint;
        // Reserve space for hint + fwd arrow
        let reserved = hint.len() + fwd_arrow.len() + 1;
        let max_status = (area.width as usize).saturating_sub(reserved + back_arrow.len());

        for (i, c) in status.chars().take(max_status).enumerate() {
            if x + i as u16 >= area.x + area.width { break; }
            buf[(x + i as u16, area.y)].set_char(c).set_style(status_style);
        }

        // Right side: key hints
        let hint_style = Style::default().bg(Color::DarkGray).fg(Color::Gray);
        let hint_start = area.x + area.width
            - hint.len() as u16
            - fwd_arrow.len() as u16;

        for (i, c) in hint.chars().enumerate() {
            let px = hint_start + i as u16;
            if px < area.x + area.width {
                buf[(px, area.y)].set_char(c).set_style(hint_style);
            }
        }

        // Forward arrow at the very right
        let fwd_start = area.x + area.width - fwd_arrow.len() as u16;
        for (i, c) in fwd_arrow.chars().enumerate() {
            buf[(fwd_start + i as u16, area.y)].set_char(c)
                .set_style(if self.can_go_forward { nav_style_active } else { nav_style_dim });
        }
    }
}
