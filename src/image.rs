use anyhow::{anyhow, Result};
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

pub struct ChafaRenderer {
    pub available: bool,
    pub use_sixel: bool,
}

impl ChafaRenderer {
    pub fn new() -> Self {
        let available = which_chafa();
        let use_sixel = available && supports_sixel();
        Self { available, use_sixel }
    }

    pub fn render_image(
        &self,
        image_bytes: &[u8],
        width_cols: u16,
        height_rows: u16,
    ) -> Result<String> {
        if !self.available {
            return Err(anyhow!("chafa not available"));
        }

        // Write image bytes to a temp file
        let mut tmpfile = NamedTempFile::new()?;
        tmpfile.write_all(image_bytes)?;
        tmpfile.flush()?;
        let path = tmpfile.path().to_string_lossy().to_string();

        let format = if self.use_sixel { "sixel" } else { "symbols" };
        let size = format!("{}x{}", width_cols, height_rows);

        let output = Command::new("chafa")
            .args([
                "--format", format,
                "--size", &size,
                "--animate", "false",
                &path,
            ])
            .output()?;

        if !output.status.success() {
            return Err(anyhow!("chafa exited with status {}", output.status));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

fn which_chafa() -> bool {
    Command::new("which")
        .arg("chafa")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn supports_sixel() -> bool {
    // Don't use sixel inside tmux (it doesn't render without allow-passthrough)
    if std::env::var("TMUX").is_ok() {
        return false;
    }
    let term = std::env::var("TERM").unwrap_or_default();
    let colorterm = std::env::var("COLORTERM").unwrap_or_default();
    // Sixel works in xterm, mlterm, foot, WezTerm, etc.
    term.contains("xterm") || term.contains("256color")
        || colorterm == "truecolor" || colorterm == "24bit"
}
