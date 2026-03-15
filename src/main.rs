mod app;
mod browser;
mod fetcher;
mod image;
mod keybindings;
mod parser;
mod renderer;
mod ui;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;
use url::Url;

use app::App;

#[tokio::main]
async fn main() -> Result<()> {
    // ── Logging to file (stdout is the TUI) ──────────────────────────────────
    let file_appender = rolling::never("/tmp", "shellscape.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // ── Parse optional URL argument ───────────────────────────────────────────
    let initial_url: Option<Url> = std::env::args().nth(1).and_then(|arg| {
        // Try as-is, then prepend https://
        Url::parse(&arg).ok()
            .or_else(|| Url::parse(&format!("https://{}", arg)).ok())
    });

    // ── Panic hook: restore terminal before printing ──────────────────────────
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stderr(), LeaveAlternateScreen);
        default_hook(info);
    }));

    // ── Terminal setup ────────────────────────────────────────────────────────
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // ── Run app ───────────────────────────────────────────────────────────────
    let result = run(&mut terminal, initial_url).await;

    // ── Terminal teardown ─────────────────────────────────────────────────────
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    initial_url: Option<Url>,
) -> Result<()> {
    let mut app = App::new(initial_url)?;
    app.run(terminal).await
}
