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
    pub mode_hint: &'a str,
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 { return; }

        // Fill background
        let bar_style = Style::default().bg(Color::DarkGray).fg(Color::White);
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_char(' ').set_style(bar_style);
        }

        // Left side: status
        let status = match self.load_state {
            LoadState::Idle => " Ready".to_string(),
            LoadState::Loading { url } => {
                let spinner = SPINNER[self.spinner_tick % SPINNER.len()];
                format!(" {} Loading {}…", spinner, url)
            }
            LoadState::Error(e) => format!(" ✗ {}", e),
        };

        let status_style = match self.load_state {
            LoadState::Error(_) => Style::default().bg(Color::Red).fg(Color::White)
                .add_modifier(Modifier::BOLD),
            LoadState::Loading { .. } => Style::default().bg(Color::DarkGray).fg(Color::Yellow),
            _ => bar_style,
        };

        let status_chars: Vec<char> = status.chars().collect();
        let max_status = (area.width as usize).saturating_sub(self.mode_hint.len() + 2);
        for (i, &c) in status_chars.iter().take(max_status).enumerate() {
            buf[(area.x + i as u16, area.y)].set_char(c).set_style(status_style);
        }

        // Right side: key hints
        let hint_style = Style::default().bg(Color::DarkGray).fg(Color::Gray);
        let hint = self.mode_hint;
        let hint_x = area.x + area.width - hint.len() as u16;
        for (i, c) in hint.chars().enumerate() {
            if hint_x + (i as u16) < area.x + area.width {
                buf[(hint_x + i as u16, area.y)].set_char(c).set_style(hint_style);
            }
        }
    }
}
