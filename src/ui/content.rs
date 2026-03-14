use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget},
};

use crate::browser::{RenderedPage, StyledLine};

use super::splash::SplashScreen;

pub struct ContentArea<'a> {
    pub page: Option<&'a RenderedPage>,
    pub scroll_offset: usize,
    pub focused_link: Option<usize>,
}

impl<'a> Widget for ContentArea<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 { return; }

        let Some(page) = self.page else {
            SplashScreen.render(area, buf);
            return;
        };

        let viewport_height = area.height as usize;
        let visible_lines = page.lines
            .iter()
            .skip(self.scroll_offset)
            .take(viewport_height);

        let ratatui_lines: Vec<Line<'_>> = visible_lines
            .map(|sl| styled_line_to_ratatui(sl, self.focused_link))
            .collect();

        let text = Text::from(ratatui_lines);
        let para = Paragraph::new(text);

        // Reserve 1 col for scrollbar
        let content_area = Rect { width: area.width.saturating_sub(1), ..area };
        para.render(content_area, buf);

        // Scrollbar
        let total = page.lines.len();
        if total > viewport_height {
            let mut scrollbar_state = ScrollbarState::new(total)
                .position(self.scroll_offset);
            let scrollbar_area = Rect {
                x: area.x + area.width - 1,
                ..area
            };
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .render(scrollbar_area, buf, &mut scrollbar_state);
        }
    }
}

fn styled_line_to_ratatui<'a>(line: &'a StyledLine, focused_link: Option<usize>) -> Line<'a> {
    let spans: Vec<Span<'a>> = line.spans.iter().map(|span| {
        let mut style = span.style;
        if let (Some(focused), Some(link_idx)) = (focused_link, span.link_idx) {
            if focused == link_idx {
                style = style.add_modifier(Modifier::REVERSED);
            }
        }
        Span::styled(span.text.clone(), style)
    }).collect();
    Line::from(spans)
}
