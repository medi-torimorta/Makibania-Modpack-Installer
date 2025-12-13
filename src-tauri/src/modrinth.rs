use std::sync::LazyLock;

use anyhow::{anyhow, Result};
use ferinth::Ferinth;

static FERINTH: LazyLock<Ferinth<()>> = LazyLock::new(|| {
    Ferinth::<()>::new(
        env!("CARGO_PKG_NAME"),
        Some(env!("CARGO_PKG_VERSION")),
        None,
    )
});

pub struct Modrinth;

impl Modrinth {
    pub async fn get_download_url(project_id: &str, version_id: &str) -> Result<String> {
        let version = FERINTH.version_get(version_id).await?;
        if version.project_id != project_id {
            return Err(anyhow!(
                "Project ID mismatch: expected {}, got {}.",
                project_id,
                version.project_id
            ));
        }
        let file = version
            .files
            .iter()
            .find(|f| f.primary)
            .or_else(|| version.files.first())
            .ok_or_else(|| {
                anyhow!(
                    "No files found for version {} of project {}.",
                    version_id,
                    project_id
                )
            })?;

        Ok(file.url.to_string())
    }
}
