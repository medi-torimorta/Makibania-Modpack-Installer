use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use reqwest::{Client, Response};
use sha1::{Digest, Sha1};
use urlencoding;

const DOWNLOAD_TIMEOUT_SECS: u64 = 10;

#[derive(Clone)]
pub struct DownloadManager {
    client: Client,
}

#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
}

impl DownloadManager {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
            .build()
            .context("Failed to build HTTP client")?;
        Ok(Self { client })
    }

    pub async fn download_to_dir<F>(
        &self,
        url: &str,
        temp_dir: &Path,
        mut progress_callback: Option<F>,
    ) -> Result<DownloadOutcome>
    where
        F: FnMut(DownloadProgress) -> Result<()>,
    {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed to download from {url}"))?;
        let response = ensure_success(response, url).await?;
        let file_name = extract_file_name(&response)?;
        let destination = temp_dir.join(&file_name);
        fs::create_dir_all(temp_dir)
            .with_context(|| format!("Failed to create directory {}", temp_dir.display()))?;
        let mut file = File::create(&destination).with_context(|| {
            format!(
                "Failed to create destination file {}",
                destination.display()
            )
        })?;
        let total_bytes = response.content_length();
        let mut received_bytes = 0u64;
        let mut hasher = Sha1::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.with_context(|| format!("Failed to read chunk from {url}"))?;
            file.write_all(&chunk)?;
            hasher.update(&chunk);
            received_bytes += chunk.len() as u64;
            if let Some(callback) = progress_callback.as_mut() {
                callback(DownloadProgress {
                    received_bytes,
                    total_bytes,
                })?;
            }
        }
        file.flush()?;
        let hash_bytes = hasher.finalize();
        let hash = hex::encode(hash_bytes);

        Ok(DownloadOutcome {
            path: destination.to_path_buf(),
            hash,
        })
    }
}

fn extract_file_name(response: &Response) -> Result<String> {
    // Try Content-Disposition header first
    if let Some(content_disposition) = response.headers().get("content-disposition") {
        if let Ok(header_value) = content_disposition.to_str() {
            // Parse: attachment; filename="example.jar"
            for part in header_value.split(';') {
                let part = part.trim();
                if let Some(file_name) = part.strip_prefix("filename=") {
                    let file_name = file_name.trim_matches('"').trim();
                    if !file_name.is_empty() {
                        return Ok(file_name.to_string());
                    }
                }
            }
        }
    }
    let url = response.url();
    // Fallback: extract from final URL path (after redirects)
    if let Some(segments) = url.path_segments() {
        if let Some(last_segment) = segments.last() {
            if !last_segment.is_empty() {
                // Remove query parameters if present
                let file_name = last_segment.split('?').next().unwrap_or(last_segment);
                if !file_name.is_empty() {
                    return Ok(urlencoding::decode(file_name)?.to_string());
                }
            }
        }
    }
    bail!("Could not determine file name from final URL '{url}' or response headers")
}

async fn ensure_success(response: Response, url: &str) -> Result<Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body_snippet = match response.text().await {
        Ok(body) => {
            if body.is_empty() {
                "<empty body>".to_string()
            } else if body.len() > 512 {
                format!("{}...", &body[..512])
            } else {
                body
            }
        }
        Err(_) => "<failed to read body>".to_string(),
    };
    bail!("Request to {url} failed with status {status}. Body snippet: {body_snippet}");
}

#[derive(Debug)]
pub struct DownloadOutcome {
    pub path: PathBuf,
    pub hash: String,
}
