use anyhow::{anyhow, Result};
use reqwest::{Client, Response};

pub async fn run_get_request(
    url: &str,
    token: Option<&String>,
    headers: Vec<(String, String)>,
) -> Result<Response> {
    let mut res = match token {
        Some(token) => Client::new().get(url).bearer_auth(token),
        None => Client::new().get(url),
    };

    for (key, value) in headers {
        res = res.header(key, value);
    }

    let res = res
        .send()
        .await
        .map_err(|e| anyhow!(e).context(format!("Failed to run request to {}", url)))?;

    if !res.status().is_success() {
        return Err(anyhow!(res.text().await.unwrap_or_default())
            .context(format!("Failed to get a success response from {}", url)));
    }

    Ok(res)
}
