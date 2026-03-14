# shellscape

A terminal-based web browser with ASCII/Unicode graphics, written in Rust.

```
 ███████╗██╗  ██╗███████╗██╗     ██╗     ███████╗ ██████╗ █████╗ ██████╗ ███████╗
 ██╔════╝██║  ██║██╔════╝██║     ██║     ██╔════╝██╔════╝██╔══██╗██╔══██╗██╔════╝
 ███████╗███████║█████╗  ██║     ██║     ███████╗██║     ███████║██████╔╝█████╗
 ╚════██║██╔══██║██╔══╝  ██║     ██║     ╚════██║██║     ██╔══██║██╔═══╝ ██╔══╝
 ███████║██║  ██║███████╗███████╗███████╗███████║╚██████╗██║  ██║██║     ███████╗
 ╚══════╝╚═╝  ╚═╝╚══════╝╚══════╝╚══════╝╚══════╝ ╚═════╝╚═╝  ╚═╝╚═╝     ╚══════╝
```

## Features

- Full TUI browser built with [Ratatui](https://ratatui.rs)
- HTML parsing via `html5ever` — renders headings, paragraphs, lists, tables, blockquotes, code blocks, and many inline elements
- Images rendered as high-quality color ASCII art via [Chafa](https://hpjansson.org/chafa/) — automatically selects the best format for your terminal (truecolor symbols, 256-color, sixel, Kitty, iTerm2)
- HTTPS with rustls — no native OpenSSL dependency
- Async HTTP via `reqwest` + `tokio` with gzip/brotli/deflate decompression
- Vi-style keybindings
- Multi-tab support
- Browser history with back/forward navigation
- Mouse scroll support
- Page text reflows when the terminal is resized
- Logs to `/tmp/shellscape.log` so stdout stays clean for the TUI

---

## Requirements

### Rust

Rust 1.80 or later. Install via [rustup](https://rustup.rs):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Chafa (optional, for image rendering)

[Chafa](https://hpjansson.org/chafa/) 1.6 or later is required for image rendering. Without it the browser still works — images show as `[IMG: alt text]` placeholders.

**macOS:**
```sh
brew install chafa
```

**Debian / Ubuntu:**
```sh
sudo apt install chafa
```

**Arch Linux:**
```sh
sudo pacman -S chafa
```

**Fedora / RHEL:**
```sh
sudo dnf install chafa
```

**From source** (for the latest version with all quality flags):
```sh
git clone https://github.com/hpjansson/chafa.git
cd chafa && ./autogen.sh && make && sudo make install
```

Chafa 1.10+ unlocks Bayer dithering (`--dither bayer`) for smoother image gradients. Chafa 1.6+ unlocks `--work 9` and `--color-space din99d` for maximum quality.

---

## Building

```sh
git clone https://github.com/sadovsky/shellscape.git
cd shellscape
cargo build --release
```

The compiled binary will be at `target/release/shellscape`.

To run directly without installing:

```sh
cargo run --release
```

### Running tests

```sh
cargo test
```

---

## Installation

Copy the binary somewhere on your `$PATH`:

```sh
sudo cp target/release/shellscape /usr/local/bin/
```

Or install via Cargo:

```sh
cargo install --path .
```

---

## Usage

### Open the browser (splash screen)

```sh
shellscape
```

### Open a URL directly

```sh
shellscape https://example.com
shellscape news.ycombinator.com   # https:// is added automatically
```

---

## Keybindings

### Navigation

| Key | Action |
|-----|--------|
| `o` | Open URL (address bar) |
| `Enter` | Follow focused link |
| `l` | Follow focused link |
| `H` | Go back |
| `L` | Go forward |
| `r` | Reload current page |

### Scrolling

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down one line |
| `k` / `↑` | Scroll up one line |
| `d` | Scroll down half page |
| `u` | Scroll up half page |
| `gg` | Jump to top |
| `G` | Jump to bottom |
| Mouse wheel | Scroll up / down |

### Links

| Key | Action |
|-----|--------|
| `Tab` | Focus next link |
| `Shift+Tab` | Focus previous link |
| `Enter` / `l` | Follow focused link |

### Tabs

| Key | Action |
|-----|--------|
| `t` | New tab |
| `x` | Close current tab |
| `1`–`9` | Switch to tab N |
| `Ctrl+h` | Previous tab |
| `Ctrl+l` | Next tab |

### Search

| Key | Action |
|-----|--------|
| `/` | Start search |
| `Esc` | Cancel |

### Address bar

| Key | Action |
|-----|--------|
| `o` | Open address bar |
| `Enter` | Navigate to typed URL |
| `Esc` | Cancel |
| `Ctrl+a` | Move cursor to start |
| `Ctrl+e` | Move cursor to end |
| `←` / `→` | Move cursor |
| `Backspace` | Delete character before cursor |

### General

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Ctrl+c` | Quit |

---

## Image Rendering

Shellscape detects your terminal's capabilities and picks the best image format automatically:

| Format | Quality | Required |
|--------|---------|----------|
| Kitty graphics protocol | Pixel-perfect | Kitty terminal |
| iTerm2 inline images | Pixel-perfect | iTerm2 / WezTerm |
| Sixel | Near pixel-perfect | xterm, mlterm, foot, etc. |
| Symbols + truecolor | High-quality color ASCII | `$COLORTERM=truecolor` |
| Symbols + 256-color | Good color ASCII | 256-color terminal |
| Symbols + 16-color | Basic ASCII | Any terminal |

For the best color ASCII experience, use a truecolor terminal (Kitty, Alacritty, WezTerm, or any terminal with `$COLORTERM=truecolor`) and install Chafa 1.10+.

Image rendering flags used (symbols/truecolor mode):
```
--colors full          24-bit RGB color
--color-space din99d   Perceptual color accuracy
--symbols all          Full Unicode: block, braille, border chars
--work 9               Maximum quality computation
--font-ratio 0.5       Correct terminal cell aspect ratio
--dither bayer         Smooth gradient dithering (chafa ≥1.10)
```

---

## HTML Elements Supported

**Block elements:** `h1`–`h6`, `p`, `div`, `section`, `article`, `main`, `nav`, `aside`, `header`, `footer`, `blockquote`, `pre`, `ul`, `ol`, `li`, `dl`/`dt`/`dd`, `table`, `figure`/`figcaption`, `details`/`summary`, `address`, `hr`

**Inline elements:** `a`, `strong`/`b`, `em`/`i`, `code`, `del`/`s`, `ins`/`u`, `mark`, `kbd`, `sub`, `sup`, `abbr`, `q`, `small`, `cite`, `var`, `samp`, `time`

**Images:** `img` with async download and Chafa rendering; `alt` text shown as placeholder until render completes

---

## Architecture

```
src/
├── main.rs          Entry point — tokio runtime, terminal setup, panic hook
├── app.rs           App state + async event loop (tokio::select!)
├── browser.rs       BrowserState, Tab, history, scroll, rendered page types
├── fetcher.rs       Async HTTP (reqwest), redirect following, content-type detection
├── parser.rs        html5ever → owned DomNode IR, relative URL resolution
├── renderer.rs      DomNode → Vec<StyledLine> layout engine
├── image.rs         Terminal capability detection, Chafa integration,
│                    ANSI SGR parser → StyledLine
├── keybindings.rs   Vi-style KeyEvent → Action mapping
├── error.rs         Error types
└── ui/
    ├── mod.rs        Top-level draw() + Ratatui layout
    ├── splash.rs     ASCII art splash screen
    ├── address_bar.rs URL input with mode-aware label + sliding cursor
    ├── content.rs    Scrollable content area + scrollbar + link highlights
    ├── status_bar.rs  Status, spinner, back/forward arrows, key hints
    └── tabs.rs       Tab bar widget
```

The event loop multiplexes three sources with `tokio::select!`: crossterm terminal events, async fetch results, and async image render results. HTTP fetches and Chafa image renders each run in their own `tokio::spawn` task and send results back via an unbounded mpsc channel.

---

## Logging

All log output goes to `/tmp/shellscape.log` so it doesn't corrupt the TUI. To enable verbose logging:

```sh
RUST_LOG=shellscape=debug shellscape
```

---

## License

MIT — see [LICENSE](LICENSE).
