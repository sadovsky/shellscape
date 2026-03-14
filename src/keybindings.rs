use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone)]
pub enum Action {
    // Scrolling
    ScrollDown(usize),
    ScrollUp(usize),
    ScrollTop,
    ScrollBottom,
    ScrollPageDown,
    ScrollPageUp,

    // Link navigation
    FocusNextLink,
    FocusPrevLink,
    FollowLink,

    // Browser navigation
    GoBack,
    GoForward,
    Reload,

    // Modes
    EnterAddressBar,
    ExitMode,
    SubmitInput,
    SearchStart,
    SearchNext,
    SearchPrev,

    // Input
    InputChar(char),
    InputBackspace,
    InputDelete,
    InputLeft,
    InputRight,
    InputHome,
    InputEnd,

    // Tabs
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    SwitchTab(usize),

    // App
    Quit,
}

/// Maps a crossterm KeyEvent to an Action in Normal mode.
pub fn map_normal(key: KeyEvent) -> Option<Action> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('c') if ctrl => Some(Action::Quit),

        KeyCode::Char('j') | KeyCode::Down => Some(Action::ScrollDown(1)),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::ScrollUp(1)),
        KeyCode::Char('d') => Some(Action::ScrollPageDown),
        KeyCode::Char('u') => Some(Action::ScrollPageUp),
        KeyCode::Char('G') => Some(Action::ScrollBottom),
        // 'g' handled by double-key state machine in app.rs

        KeyCode::Tab => Some(Action::FocusNextLink),
        KeyCode::BackTab => Some(Action::FocusPrevLink),
        KeyCode::Enter | KeyCode::Char('l') => Some(Action::FollowLink),

        KeyCode::Char('H') => Some(Action::GoBack),
        KeyCode::Char('L') => Some(Action::GoForward),
        KeyCode::Char('r') => Some(Action::Reload),

        KeyCode::Char('o') => Some(Action::EnterAddressBar),
        KeyCode::Char('/') => Some(Action::SearchStart),
        KeyCode::Char('n') => Some(Action::SearchNext),
        KeyCode::Char('N') => Some(Action::SearchPrev),

        KeyCode::Char('t') => Some(Action::NewTab),
        KeyCode::Char('x') => Some(Action::CloseTab),
        KeyCode::Char('h') if ctrl => Some(Action::PrevTab),
        // Ctrl+L for next tab handled separately (avoids overlap with FollowLink 'l')

        KeyCode::Char(c @ '1'..='9') => {
            Some(Action::SwitchTab(c as usize - '1' as usize))
        }

        _ => None,
    }
}

/// Maps a KeyEvent in input mode (address bar or search).
pub fn map_input(key: KeyEvent) -> Option<Action> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => Some(Action::ExitMode),
        KeyCode::Enter => Some(Action::SubmitInput),
        KeyCode::Backspace => Some(Action::InputBackspace),
        KeyCode::Delete => Some(Action::InputDelete),
        KeyCode::Left => Some(Action::InputLeft),
        KeyCode::Right => Some(Action::InputRight),
        KeyCode::Home | KeyCode::Char('a') if ctrl => Some(Action::InputHome),
        KeyCode::End | KeyCode::Char('e') if ctrl => Some(Action::InputEnd),
        KeyCode::Char('c') if ctrl => Some(Action::Quit),
        KeyCode::Char(c) => Some(Action::InputChar(c)),
        _ => None,
    }
}
