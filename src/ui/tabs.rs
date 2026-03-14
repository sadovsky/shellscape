use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

pub struct TabBar<'a> {
    pub tab_titles: &'a [String],
    pub active: usize,
}

impl<'a> Widget for TabBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || self.tab_titles.is_empty() { return; }

        let active_style = Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let inactive_style = Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray);
        let sep_style = Style::default().fg(Color::DarkGray).bg(Color::Reset);

        let mut x = area.x;
        for (i, title) in self.tab_titles.iter().enumerate() {
            let style = if i == self.active { active_style } else { inactive_style };
            let label = format!(" {} ", title);
            for c in label.chars() {
                if x >= area.x + area.width { break; }
                buf[(x, area.y)].set_char(c).set_style(style);
                x += 1;
            }
            if x < area.x + area.width {
                buf[(x, area.y)].set_char('│').set_style(sep_style);
                x += 1;
            }
        }

        // Fill remaining with background
        let bg_style = Style::default().bg(Color::Reset);
        while x < area.x + area.width {
            buf[(x, area.y)].set_char(' ').set_style(bg_style);
            x += 1;
        }
    }
}
