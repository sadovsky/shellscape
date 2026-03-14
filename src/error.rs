use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShellscapeError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Render error: {0}")]
    Render(String),

    #[error("Chafa unavailable: images will show alt text only")]
    ChafaUnavailable,
}
