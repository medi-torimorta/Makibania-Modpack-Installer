use std::{
    collections::HashMap,
    fs,
    path::{Component, Path},
};

use anyhow::{bail, Context, Result};
use semver::Version;
use serde::{Deserialize, Serialize};

pub const LATEST_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModPackConfig {
    schema_version: u32,
    pack_version: Version,
    profile: Profile,
    mod_loader: ModLoader,
    #[serde(default)]
    mods: Vec<ModEntry>,
    #[serde(default)]
    resources: Vec<ResourceEntry>,

    #[serde(skip)]
    mod_index: HashMap<String, usize>,
}

impl ModPackConfig {
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file at {}", path.display()))?;
        let mut config: ModPackConfig =
            serde_yaml::from_str(&raw).context("Failed to parse config.yaml")?;
        config.validate()?;
        // Build indexes
        debug_assert!(config.mod_index.is_empty());
        for (i, mod_entry) in config.mods.iter().enumerate() {
            let key = Self::mod_key(&mod_entry.source);
            config.mod_index.insert(key, i);
        }
        Ok(config)
    }

    pub fn get_pack_version(&self) -> &Version {
        &self.pack_version
    }

    pub fn get_profile(&self) -> &Profile {
        &self.profile
    }

    pub fn get_mod_loader(&self) -> &ModLoader {
        &self.mod_loader
    }

    pub fn has_mod(&self, source: &SourceType) -> bool {
        self.mod_index.contains_key(&Self::mod_key(source))
    }

    pub fn get_mods(&self) -> &Vec<ModEntry> {
        &self.mods
    }

    pub fn get_resources(&self) -> &Vec<ResourceEntry> {
        &self.resources
    }

    fn validate(&mut self) -> Result<()> {
        if self.schema_version > LATEST_SCHEMA_VERSION {
            bail!(
                "Unsupported config schema version '{}' (expected version {} or lower)",
                self.schema_version,
                LATEST_SCHEMA_VERSION
            );
        }
        self.profile.validate()?;
        self.mod_loader.validate()?;
        for entry in self.mods.iter_mut() {
            entry.validate()?;
        }
        for entry in self.resources.iter_mut() {
            entry.validate()?;
        }

        Ok(())
    }

    fn mod_key(source: &SourceType) -> String {
        match source {
            SourceType::Curseforge {
                project_id,
                file_id,
            } => format!("cf:{project_id}:{file_id}"),
            SourceType::Direct { url } => format!("direct:{url}"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    pub icon: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jvm_args: Option<String>,
}

impl Profile {
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("profile.name must not be empty");
        }
        if self.version.trim().is_empty() {
            bail!("profile.version must not be empty");
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModLoader {
    pub name: String,
    pub url: String,
    pub hash: String,
    #[serde(default)]
    pub auto_open: bool,
}

impl ModLoader {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    pub name: String,
    #[serde(flatten)]
    pub source: SourceType,
    pub hash: String,
    pub side: SideType,
}

impl ModEntry {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceEntry {
    pub name: String,
    #[serde(flatten)]
    pub source: SourceType,
    pub hash: String,
    pub target_dir: String,
    #[serde(default)]
    pub decompress: bool,
    pub side: SideType,
}

impl ResourceEntry {
    fn validate(&self) -> Result<()> {
        validate_relative_dir(&self.target_dir, "resources.targetDir")?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum SourceType {
    Curseforge { project_id: u32, file_id: u32 },
    Direct { url: String },
}

impl SourceType {
    pub fn get_download_url(&self) -> String {
        match self {
            SourceType::Curseforge {
                project_id,
                file_id,
            } => format!(
                "https://www.curseforge.com/api/v1/mods/{project_id}/files/{file_id}/download"
            ),
            SourceType::Direct { url } => url.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SideType {
    Both,
    Client,
    Server,
}

fn validate_relative_dir(dir: &str, field: &str) -> Result<()> {
    let path = Path::new(dir);
    if path.is_absolute() {
        bail!("{field} must be a relative path");
    }
    for component in path.components() {
        match component {
            Component::ParentDir => {
                bail!("{field} must not contain '..' segments");
            }
            Component::Normal(segment) => {
                if segment.to_string_lossy().contains(['\\', ':']) {
                    bail!("{field} contains invalid characters");
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                bail!("{field} must be a relative path");
            }
            Component::CurDir => {}
        }
    }
    Ok(())
}
