use anyhow::{anyhow, Result};
use reqwest::Client;
use url::Url;

#[derive(Debug)]
pub struct FetchResult {
    pub url: Url,
    pub status: u16,
    pub content_type: String,
    pub body: FetchBody,
}

#[derive(Debug)]
pub enum FetchBody {
    Html(String),
    Binary { mime: String, bytes: Vec<u8> },
}

const MAX_BODY_BYTES: usize = 10 * 1024 * 1024; // 10 MB
const USER_AGENT: &str = "shellscape/0.1 (terminal browser; +https://github.com/sadovsky/shellscape)";

pub async fn fetch(client: &Client, url: Url) -> Result<FetchResult> {
    let response = client
        .get(url.as_str())
        .header("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await?;

    let status = response.status().as_u16();
    let final_url = Url::parse(response.url().as_str())?;

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/html")
        .to_string();

    let is_html = content_type.contains("text/html")
        || content_type.contains("application/xhtml");

    let bytes = response.bytes().await?;
    if bytes.len() > MAX_BODY_BYTES {
        return Err(anyhow!("Response body too large ({} bytes)", bytes.len()));
    }

    let body = if is_html {
        let text = String::from_utf8_lossy(&bytes).into_owned();
        FetchBody::Html(text)
    } else {
        FetchBody::Binary {
            mime: content_type.clone(),
            bytes: bytes.to_vec(),
        }
    };

    Ok(FetchResult { url: final_url, status, content_type, body })
}

pub async fn fetch_bytes(client: &Client, url: Url) -> Result<Vec<u8>> {
    let bytes = client
        .get(url.as_str())
        .header("User-Agent", USER_AGENT)
        .send()
        .await?
        .bytes()
        .await?;
    Ok(bytes.to_vec())
}

pub fn build_client() -> Result<Client> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;
    Ok(client)
}
