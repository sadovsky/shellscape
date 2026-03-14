use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::Widget,
};

pub struct AddressBar<'a> {
    pub url: &'a str,
    pub input: &'a str,
    pub cursor_pos: usize,
    pub editing: bool,
}

impl<'a> Widget for AddressBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 { return; }

        let label_style = Style::default().fg(Color::DarkGray);
        let url_style = Style::default().fg(Color::White);
        let input_style = Style::default().fg(Color::Yellow);
        let cursor_style = Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD);

        let prefix = " URL: ";
        let mut x = area.x;
        let y = area.y;

        // Draw prefix
        let _prefix_span = Span::styled(prefix, label_style);
        for (i, c) in prefix.chars().enumerate() {
            if x + i as u16 >= area.x + area.width { break; }
            buf[(x + i as u16, y)].set_char(c).set_style(label_style);
        }
        x += prefix.len() as u16;

        if self.editing {
            // Draw input with cursor
            let input_chars: Vec<char> = self.input.chars().collect();
            for (i, &c) in input_chars.iter().enumerate() {
                if x + i as u16 >= area.x + area.width { break; }
                let style = if i == self.cursor_pos { cursor_style } else { input_style };
                buf[(x + i as u16, y)].set_char(c).set_style(style);
            }
            // Draw cursor at end if past all chars
            let end_x = x + input_chars.len() as u16;
            if self.cursor_pos >= input_chars.len() && end_x < area.x + area.width {
                buf[(end_x, y)].set_char(' ').set_style(cursor_style);
            }
        } else {
            // Draw current URL
            for (i, c) in self.url.chars().enumerate() {
                if x + i as u16 >= area.x + area.width { break; }
                buf[(x + i as u16, y)].set_char(c).set_style(url_style);
            }
        }
    }
}
