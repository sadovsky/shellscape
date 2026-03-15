use anyhow::{anyhow, Result};
use ratatui::style::{Color, Modifier, Style};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

use crate::browser::{LineType, StyledLine, StyledSpan};

// ── Image quality mode ───────────────────────────────────────────────────────

/// Controls how images are rendered via chafa.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ImageQuality {
    /// Unicode block/braille characters with full color (default).
    #[default]
    Color,
    /// Classic monochrome ASCII art (`@`, `#`, `.`, etc.).
    Ascii,
}

// ── Terminal capabilities ────────────────────────────────────────────────────

/// Detected terminal capabilities relevant to image rendering.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TermCapabilities {
    /// Kitty terminal graphics protocol (pixel-perfect, best quality)
    pub kitty: bool,
    /// iTerm2 / WezTerm inline image protocol
    pub iterm2: bool,
    /// Sixel raster graphics
    pub sixel: bool,
    /// 24-bit truecolor ANSI support
    pub truecolor: bool,
    /// 256-color ANSI support
    pub color_256: bool,
    /// Running inside tmux (limits pixel protocols without allow-passthrough)
    pub in_tmux: bool,
}

impl TermCapabilities {
    pub fn detect() -> Self {
        let in_tmux = std::env::var("TMUX").is_ok();
        let term = std::env::var("TERM").unwrap_or_default();
        let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
        let colorterm = std::env::var("COLORTERM").unwrap_or_default();
        let kitty_id = std::env::var("KITTY_WINDOW_ID").is_ok();

        // Kitty: KITTY_WINDOW_ID set, or $TERM == xterm-kitty
        // Not usable in tmux without allow-passthrough
        let kitty = !in_tmux && (kitty_id || term == "xterm-kitty");

        // iTerm2 / WezTerm inline images
        let iterm2 = !in_tmux
            && matches!(
                term_program.as_str(),
                "iTerm.app" | "WezTerm" | "mintty"
            );

        // Truecolor: $COLORTERM=truecolor/24bit, or known truecolor terminals
        let truecolor = matches!(colorterm.as_str(), "truecolor" | "24bit")
            || term.contains("kitty")
            || term.contains("alacritty")
            || term_program == "WezTerm"
            || term_program == "iTerm.app";

        // 256-color
        let color_256 = truecolor || term.contains("256color") || term.contains("xterm");

        // Sixel: a small set of terminals reliably support it.
        // In tmux we conservatively disable unless we know allow-passthrough is on.
        let sixel = !in_tmux
            && matches!(
                term.as_str(),
                "xterm" | "xterm-256color" | "mlterm" | "mlterm-256color"
                    | "foot" | "foot-extra" | "contour" | "yaft" | "yaft-256color"
            );

        Self { kitty, iterm2, sixel, truecolor, color_256, in_tmux }
    }

    /// True when the best available format produces pixel/raster output rather
    /// than Unicode block characters (kitty, iterm2, sixel).
    #[allow(dead_code)]
    pub fn uses_pixel_format(&self) -> bool {
        self.kitty || self.iterm2 || self.sixel
    }

    /// Returns chafa `--format` value and associated colour-quality flags for
    /// the best format this terminal supports.
    #[allow(dead_code)]
    pub fn chafa_format_flags(&self) -> Vec<String> {
        if self.kitty {
            return vec!["--format".into(), "kitty".into()];
        }
        if self.iterm2 {
            return vec!["--format".into(), "iterm".into()];
        }
        if self.sixel {
            return vec!["--format".into(), "sixel".into()];
        }
        self.symbols_format_flags()
    }

    /// Returns chafa flags for classic monochrome ASCII art rendering
    /// using the printable ASCII character set with no color.
    pub fn ascii_format_flags(&self) -> Vec<String> {
        vec![
            "--format".into(), "symbols".into(),
            "--symbols".into(), "ascii".into(),
            "--colors".into(),  "none".into(),
        ]
    }

    /// Returns chafa flags for Unicode block-character (symbols) rendering
    /// using the best available colour depth. Always produces ANSI output
    /// that can be parsed and displayed inline via Ratatui.
    pub fn symbols_format_flags(&self) -> Vec<String> {
        if self.truecolor {
            return vec![
                "--format".into(), "symbols".into(),
                "--colors".into(),  "full".into(),
                "--color-space".into(), "din99d".into(),
                "--symbols".into(), "all".into(),
            ];
        }
        if self.color_256 {
            return vec![
                "--format".into(), "symbols".into(),
                "--colors".into(),  "256".into(),
                "--color-space".into(), "din99d".into(),
                "--symbols".into(), "all".into(),
            ];
        }
        // Basic 16-color fallback
        vec![
            "--format".into(), "symbols".into(),
            "--colors".into(),  "16".into(),
            "--symbols".into(), "block+border+extra".into(),
        ]
    }
}

// ── Chafa version detection ───────────────────────────────────────────────────

/// Returns (major, minor) of the installed chafa, or (0, 0) on failure.
fn detect_chafa_version() -> (u32, u32) {
    (|| -> Option<(u32, u32)> {
        let out = Command::new("chafa")
            .arg("--version")
            .stdin(Stdio::null())
            .env_remove("TERM_PROGRAM")
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout);
        // Output looks like: "Chafa version 1.12.4"
        for word in s.split_whitespace() {
            if word.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                let parts: Vec<u32> = word.split('.')
                    .filter_map(|p| p.parse().ok())
                    .collect();
                if parts.len() >= 2 {
                    return Some((parts[0], parts[1]));
                }
            }
        }
        None
    })()
    .unwrap_or((1, 0))
}

fn which_chafa() -> bool {
    Command::new("which")
        .arg("chafa")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ── Renderer ─────────────────────────────────────────────────────────────────

pub struct ChafaRenderer {
    pub available: bool,
    pub caps: TermCapabilities,
    /// Chafa (major, minor) version, used to gate newer flags.
    pub version: (u32, u32),
}

impl ChafaRenderer {
    pub fn new() -> Self {
        let available = which_chafa();
        let caps = TermCapabilities::detect();
        let version = if available { detect_chafa_version() } else { (0, 0) };
        Self { available, caps, version }
    }

    /// Render `image_bytes` to a vec of `StyledLine`s at the given size.
    ///
    /// For pixel formats (kitty/sixel/iterm) this currently returns a single
    /// placeholder line — full passthrough rendering is a future enhancement
    /// requiring raw terminal I/O outside Ratatui's buffer model.
    ///
    /// For symbols formats the ANSI output is parsed into richly-styled spans
    /// using full truecolor when available.
    pub fn render_image(
        &self,
        image_bytes: &[u8],
        width_cols: u16,
        height_rows: u16,
        quality: ImageQuality,
    ) -> Result<Vec<StyledLine>> {
        if !self.available {
            return Err(anyhow!("chafa not available in PATH"));
        }

        let mut tmpfile = NamedTempFile::new()?;
        tmpfile.write_all(image_bytes)?;
        tmpfile.flush()?;
        let path = tmpfile.path().to_string_lossy().to_string();

        let size = format!("{}x{}", width_cols, height_rows);

        // Always use symbols mode — pixel formats (kitty/iterm2/sixel) emit binary
        // escape sequences that Ratatui cannot render inside its cell buffer model.
        let mut args: Vec<String> = match quality {
            ImageQuality::Ascii => self.caps.ascii_format_flags(),
            ImageQuality::Color => self.caps.symbols_format_flags(),
        };

        // Size
        args.extend(["--size".into(), size]);

        // Disable animation (we just want one frame)
        args.extend(["--animate".into(), "false".into()]);

        // Font ratio: terminal cells are typically ~0.5 wide:tall.
        args.extend(["--font-ratio".into(), "0.5".into()]);

        if quality == ImageQuality::Color {
            // Maximum quality computation (chafa >= 1.6)
            if self.version >= (1, 6) {
                args.extend(["--work".into(), "9".into()]);
            }
            // Bayer dithering for smoother gradients (chafa >= 1.10)
            if self.version >= (1, 10) {
                args.extend(["--dither".into(), "bayer".into()]);
            }
        }

        args.push(path);

        // Run chafa fully isolated from the live terminal:
        // - setsid() in pre_exec creates a new session with no controlling terminal,
        //   so any attempt by chafa to open /dev/tty (for OSC color queries) fails
        //   silently — preventing "rgb:xxxx/yyyy/zzzz" garbage from leaking into the TUI
        // - stdin null as an additional guard against reading terminal responses
        // - strip terminal-ID env vars so chafa skips pixel-protocol detection
        let mut cmd = Command::new("chafa");
        cmd.args(&args)
            .stdin(Stdio::null())
            .env("TERM", "xterm-256color")
            .env_remove("TERM_PROGRAM")
            .env_remove("COLORTERM")
            .env_remove("KITTY_WINDOW_ID")
            .env_remove("TMUX");
        // SAFETY: setsid() is async-signal-safe; only called in the child after fork.
        unsafe { cmd.pre_exec(|| { libc::setsid(); Ok(()) }); }
        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("chafa exited non-zero: {}", stderr.trim()));
        }

        let ansi_str = String::from_utf8_lossy(&output.stdout);
        let lines = parse_ansi_to_lines(&ansi_str);
        if lines.is_empty() {
            Err(anyhow!("chafa produced no output"))
        } else {
            Ok(lines)
        }
    }
}

// ── ANSI SGR → StyledLine parser ─────────────────────────────────────────────

/// Convert a string of ANSI-escaped text (as produced by `chafa --format symbols`)
/// into a vec of `StyledLine`s that Ratatui can render directly.
///
/// Handles:
///   - CSI SGR sequences: `\x1b[...m`  (colors, bold, etc.)
///   - CSI non-SGR sequences: ignored/skipped
///   - OSC sequences: skipped
///   - Multi-byte UTF-8 characters (Unicode block / braille glyphs)
///   - `\r\n` and bare `\n` line endings
pub fn parse_ansi_to_lines(output: &str) -> Vec<StyledLine> {
    let mut lines: Vec<StyledLine> = Vec::new();
    let mut current_spans: Vec<StyledSpan> = Vec::new();
    let mut current_style = Style::default();
    let mut current_text = String::new();

    let bytes = output.as_bytes();
    let mut i = 0;

    macro_rules! flush_span {
        () => {
            if !current_text.is_empty() {
                // Merge with the previous span if it has the same style
                if let Some(last) = current_spans.last_mut() {
                    if last.style == current_style && last.link_idx.is_none() {
                        last.text.push_str(&current_text);
                        current_text.clear();
                    } else {
                        current_spans.push(StyledSpan {
                            text: std::mem::take(&mut current_text),
                            style: current_style,
                            link_idx: None,
                        });
                    }
                } else {
                    current_spans.push(StyledSpan {
                        text: std::mem::take(&mut current_text),
                        style: current_style,
                        link_idx: None,
                    });
                }
            }
        };
    }

    macro_rules! push_line {
        () => {
            flush_span!();
            lines.push(StyledLine {
                spans: std::mem::take(&mut current_spans),
                line_type: LineType::Normal,
            });
            current_style = Style::default();
        };
    }

    while i < bytes.len() {
        match bytes[i] {
            // ESC — could be CSI (\x1b[) or OSC (\x1b]) or other
            0x1b => {
                flush_span!();
                i += 1;
                if i >= bytes.len() { break; }

                match bytes[i] {
                    // CSI sequence: \x1b[...FINAL
                    b'[' => {
                        i += 1;
                        let start = i;
                        // Collect until a final byte (0x40–0x7E)
                        while i < bytes.len() && !(0x40..=0x7E).contains(&bytes[i]) {
                            i += 1;
                        }
                        if i < bytes.len() {
                            let final_byte = bytes[i];
                            i += 1;
                            if final_byte == b'm' {
                                // SGR sequence
                                let param_str = std::str::from_utf8(&bytes[start..i - 1])
                                    .unwrap_or("");
                                current_style = apply_sgr(param_str, current_style);
                            }
                            // All other CSI sequences are silently dropped
                        }
                    }
                    // OSC sequence: \x1b]...ST  (ST = \x1b\\ or \x07)
                    b']' => {
                        i += 1;
                        while i < bytes.len() {
                            if bytes[i] == 0x07 {
                                i += 1;
                                break;
                            }
                            if bytes[i] == 0x1b
                                && i + 1 < bytes.len()
                                && bytes[i + 1] == b'\\'
                            {
                                i += 2;
                                break;
                            }
                            i += 1;
                        }
                    }
                    // Anything else after ESC: skip one byte
                    _ => { i += 1; }
                }
            }

            // Carriage return — ignore (handle \r\n as \n)
            b'\r' => { i += 1; }

            // Newline — flush current line
            b'\n' => {
                push_line!();
                i += 1;
            }

            // Regular UTF-8 text
            _ => {
                // Decode one Unicode scalar
                let rest = &bytes[i..];
                match std::str::from_utf8(rest) {
                    Ok(s) => {
                        if let Some(c) = s.chars().next() {
                            current_text.push(c);
                            i += c.len_utf8();
                        } else {
                            i += 1;
                        }
                    }
                    Err(e) => {
                        // Advance past the invalid byte sequence
                        let valid_up_to = e.valid_up_to();
                        if valid_up_to > 0 {
                            if let Ok(s) = std::str::from_utf8(&rest[..valid_up_to]) {
                                current_text.push_str(s);
                            }
                            i += valid_up_to;
                        } else {
                            i += 1; // skip one bad byte
                        }
                    }
                }
            }
        }
    }

    // Flush trailing content
    flush_span!();
    if !current_spans.is_empty() {
        lines.push(StyledLine {
            spans: current_spans,
            line_type: LineType::Normal,
        });
    }

    // Trim empty trailing lines
    while lines
        .last()
        .map_or(false, |l| l.spans.iter().all(|s| s.text.trim().is_empty()))
    {
        lines.pop();
    }

    lines
}

/// Apply one CSI SGR parameter string (e.g. `"38;2;255;0;128"`) on top of
/// `current`, returning the updated `Style`.
fn apply_sgr(params: &str, mut style: Style) -> Style {
    if params.is_empty() || params == "0" {
        return Style::reset();
    }

    // Parse all numeric parameters up-front
    let nums: Vec<u32> = params
        .split(';')
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .collect();

    let mut i = 0;
    while i < nums.len() {
        style = match nums[i] {
            0  => Style::reset(),
            1  => style.add_modifier(Modifier::BOLD),
            2  => style.add_modifier(Modifier::DIM),
            3  => style.add_modifier(Modifier::ITALIC),
            4  => style.add_modifier(Modifier::UNDERLINED),
            7  => style.add_modifier(Modifier::REVERSED),
            9  => style.add_modifier(Modifier::CROSSED_OUT),
            22 => style.remove_modifier(Modifier::BOLD | Modifier::DIM),
            23 => style.remove_modifier(Modifier::ITALIC),
            24 => style.remove_modifier(Modifier::UNDERLINED),
            27 => style.remove_modifier(Modifier::REVERSED),

            // Standard FG 30-37
            30 => style.fg(Color::Black),
            31 => style.fg(Color::Red),
            32 => style.fg(Color::Green),
            33 => style.fg(Color::Yellow),
            34 => style.fg(Color::Blue),
            35 => style.fg(Color::Magenta),
            36 => style.fg(Color::Cyan),
            37 => style.fg(Color::White),
            39 => style.fg(Color::Reset),

            // Standard BG 40-47
            40 => style.bg(Color::Black),
            41 => style.bg(Color::Red),
            42 => style.bg(Color::Green),
            43 => style.bg(Color::Yellow),
            44 => style.bg(Color::Blue),
            45 => style.bg(Color::Magenta),
            46 => style.bg(Color::Cyan),
            47 => style.bg(Color::White),
            49 => style.bg(Color::Reset),

            // Bright FG 90-97
            90  => style.fg(Color::DarkGray),
            91  => style.fg(Color::LightRed),
            92  => style.fg(Color::LightGreen),
            93  => style.fg(Color::LightYellow),
            94  => style.fg(Color::LightBlue),
            95  => style.fg(Color::LightMagenta),
            96  => style.fg(Color::LightCyan),
            97  => style.fg(Color::Gray),

            // Bright BG 100-107
            100 => style.bg(Color::DarkGray),
            101 => style.bg(Color::LightRed),
            102 => style.bg(Color::LightGreen),
            103 => style.bg(Color::LightYellow),
            104 => style.bg(Color::LightBlue),
            105 => style.bg(Color::LightMagenta),
            106 => style.bg(Color::LightCyan),
            107 => style.bg(Color::Gray),

            // Extended FG: 38;2;R;G;B  or  38;5;N
            38 if i + 1 < nums.len() => match nums[i + 1] {
                2 if i + 4 < nums.len() => {
                    let s = style.fg(Color::Rgb(
                        nums[i + 2] as u8,
                        nums[i + 3] as u8,
                        nums[i + 4] as u8,
                    ));
                    i += 4;
                    s
                }
                5 if i + 2 < nums.len() => {
                    let s = style.fg(Color::Indexed(nums[i + 2] as u8));
                    i += 2;
                    s
                }
                _ => style,
            },

            // Extended BG: 48;2;R;G;B  or  48;5;N
            48 if i + 1 < nums.len() => match nums[i + 1] {
                2 if i + 4 < nums.len() => {
                    let s = style.bg(Color::Rgb(
                        nums[i + 2] as u8,
                        nums[i + 3] as u8,
                        nums[i + 4] as u8,
                    ));
                    i += 4;
                    s
                }
                5 if i + 2 < nums.len() => {
                    let s = style.bg(Color::Indexed(nums[i + 2] as u8));
                    i += 2;
                    s
                }
                _ => style,
            },

            _ => style,
        };
        i += 1;
    }

    style
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_ansi_colors() {
        // fg red, text "hello", reset
        let input = "\x1b[31mhello\x1b[0m";
        let lines = parse_ansi_to_lines(input);
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.text, "hello");
        assert_eq!(span.style.fg, Some(Color::Red));
    }

    #[test]
    fn test_parse_truecolor() {
        // truecolor fg + bg, block character
        let input = "\x1b[38;2;255;128;0m\x1b[48;2;0;64;128m▀\x1b[0m";
        let lines = parse_ansi_to_lines(input);
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.style.fg, Some(Color::Rgb(255, 128, 0)));
        assert_eq!(span.style.bg, Some(Color::Rgb(0, 64, 128)));
        assert_eq!(span.text, "▀");
    }

    #[test]
    fn test_parse_multiline() {
        let input = "\x1b[31mred\x1b[0m\n\x1b[32mgreen\x1b[0m\n";
        let lines = parse_ansi_to_lines(input);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].spans[0].text, "red");
        assert_eq!(lines[1].spans[0].text, "green");
    }

    #[test]
    fn test_parse_256_color() {
        let input = "\x1b[38;5;196mX\x1b[0m"; // color index 196
        let lines = parse_ansi_to_lines(input);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].style.fg, Some(Color::Indexed(196)));
    }

    #[test]
    fn test_osc_stripped() {
        // OSC hyperlink — should not appear in output
        let input = "\x1b]8;;http://example.com\x07link\x1b]8;;\x07";
        let lines = parse_ansi_to_lines(input);
        assert_eq!(lines.len(), 1);
        let text: String = lines[0].spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(text, "link");
    }

    #[test]
    fn test_empty_trailing_lines_trimmed() {
        let input = "foo\n\n\n";
        let lines = parse_ansi_to_lines(input);
        // "foo" line + at most 1 blank (deduplicated by StyledLine rendering)
        // Empty trailing lines should be removed
        assert!(!lines.last().map_or(false, |l| l.spans.is_empty()
            || l.spans.iter().all(|s| s.text.trim().is_empty())));
    }

    #[test]
    fn test_merged_same_style_spans() {
        // Two consecutive same-color segments should merge into one span
        let input = "\x1b[32mhello \x1b[32mworld\x1b[0m";
        let lines = parse_ansi_to_lines(input);
        assert_eq!(lines.len(), 1);
        // Should have merged into 1 span
        assert_eq!(lines[0].spans.len(), 1);
        assert_eq!(lines[0].spans[0].text, "hello world");
    }
}
