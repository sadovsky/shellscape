use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Widget},
};

const LOGO: &[&str] = &[
    r" ███████╗██╗  ██╗███████╗██╗     ██╗     ███████╗ ██████╗ █████╗ ██████╗ ███████╗",
    r" ██╔════╝██║  ██║██╔════╝██║     ██║     ██╔════╝██╔════╝██╔══██╗██╔══██╗██╔════╝",
    r" ███████╗███████║█████╗  ██║     ██║     ███████╗██║     ███████║██████╔╝█████╗  ",
    r" ╚════██║██╔══██║██╔══╝  ██║     ██║     ╚════██║██║     ██╔══██║██╔═══╝ ██╔══╝  ",
    r" ███████║██║  ██║███████╗███████╗███████╗███████║╚██████╗██║  ██║██║     ███████╗",
    r" ╚══════╝╚═╝  ╚═╝╚══════╝╚══════╝╚══════╝╚══════╝ ╚═════╝╚═╝  ╚═╝╚═╝     ╚══════╝",
];

const TAGLINE: &str = "  A terminal web browser with ASCII graphics  ";

const HELP: &[(&str, &str)] = &[
    ("o", "Open URL"),
    ("j / k", "Scroll down / up"),
    ("d / u", "Half-page down / up"),
    ("gg / G", "Jump to top / bottom"),
    ("Tab", "Focus next link"),
    ("Enter", "Follow focused link"),
    ("H / L", "Back / Forward"),
    ("t / x", "New tab / Close tab"),
    ("1–9", "Switch to tab N"),
    ("/", "Search page"),
    ("n / N", "Next / prev search match"),
    ("r", "Reload"),
    ("q", "Quit"),
];

pub struct SplashScreen;

impl Widget for SplashScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 4 { return; }

        let logo_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let tagline_style = Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC);
        let key_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let desc_style = Style::default().fg(Color::White);
        let dim_style = Style::default().fg(Color::DarkGray);

        let mut lines: Vec<Line<'_>> = Vec::new();

        // Top padding
        lines.push(Line::from(""));

        // Logo
        for logo_line in LOGO {
            lines.push(Line::from(Span::styled(*logo_line, logo_style)));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(TAGLINE, tagline_style)));
        lines.push(Line::from(""));

        // Separator
        let sep_width = 50usize;
        let sep = format!("  {}", "─".repeat(sep_width));
        lines.push(Line::from(Span::styled(sep, dim_style)));
        lines.push(Line::from(""));

        // Keybindings
        lines.push(Line::from(Span::styled(
            "  Quick Start:",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        for (key, desc) in HELP {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:12}", key), key_style),
                Span::styled("  ", dim_style),
                Span::styled(*desc, desc_style),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Press o to get started",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )));

        let text = Text::from(lines);
        Paragraph::new(text)
            .alignment(Alignment::Left)
            .render(area, buf);
    }
}
