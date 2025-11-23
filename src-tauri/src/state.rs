use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::config::{ModEntry, ModLoader, ResourceEntry, SourceType};
use crate::installer::InstallerMode;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallerState {
    installer_version: Version,
    #[serde(default = "InstallerState::migrate_pack_version")]
    pack_version: Version,
    #[serde(default)]
    mod_loader: Option<ModLoaderState>,
    #[serde(default)]
    mods: Vec<ModState>,
    #[serde(default)]
    resources: Vec<ResourceState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    process_mode: Option<InstallerMode>,

    #[serde(skip)]
    mod_index: HashMap<String, usize>,
    #[serde(skip)]
    resource_index: HashMap<(String, String), usize>,
}

impl InstallerState {
    pub fn new(installer_version: &Version, pack_version: &Version) -> Self {
        Self {
            installer_version: installer_version.clone(),
            pack_version: pack_version.clone(),
            mod_loader: None,
            mods: Vec::new(),
            resources: Vec::new(),
            process_mode: None,
            mod_index: HashMap::new(),
            resource_index: HashMap::new(),
        }
    }

    pub fn set_installer_version(&mut self, version: &Version) {
        self.installer_version = version.clone();
    }

    pub fn get_pack_version(&self) -> &Version {
        &self.pack_version
    }

    pub fn set_pack_version(&mut self, version: &Version) {
        self.pack_version = version.clone();
    }

    pub fn get_process_mode(&self) -> Option<InstallerMode> {
        self.process_mode
    }

    pub fn set_process_mode(&mut self, mode: InstallerMode) {
        self.process_mode = Some(mode);
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

    fn resource_key(source: &SourceType, target_dir: &str) -> (String, String) {
        let source_key = match source {
            SourceType::Curseforge {
                project_id,
                file_id,
            } => format!("cf:{project_id}:{file_id}"),
            SourceType::Direct { url } => format!("direct:{url}"),
        };
        (source_key, target_dir.to_string())
    }

    pub fn get_mod_loader(&self) -> Option<&ModLoaderState> {
        self.mod_loader.as_ref()
    }

    pub fn set_mod_loader(&mut self, loader: ModLoaderState) {
        self.mod_loader = Some(loader);
    }

    pub fn get_mod_count(&self) -> usize {
        self.mods.len()
    }

    pub fn get_all_mods(&self) -> &Vec<ModState> {
        &self.mods
    }

    pub fn get_mod(&self, mod_entry: &ModEntry) -> Option<&ModState> {
        let key = Self::mod_key(&mod_entry.source);
        self.mod_index.get(&key).map(|&index| &self.mods[index])
    }

    pub fn add_mod(&mut self, mod_state: ModState) {
        let key = Self::mod_key(&mod_state.source);
        let index = self.mods.len();
        self.mods.push(mod_state);
        self.mod_index.insert(key, index);
    }

    pub fn remove_mod(&mut self, mod_state: &ModState) {
        let key = Self::mod_key(&mod_state.source);
        let Some(&index) = self.mod_index.get(&key) else {
            log::warn!(
                "Attempted to remove mod that doesn't exist in state: {}",
                mod_state.file_name
            );
            return;
        };
        self.mods.remove(index);
        self.mod_index.remove(&key);
        for i in index..self.mods.len() {
            let key = Self::mod_key(&self.mods[i].source);
            self.mod_index.insert(key, i);
        }
    }

    pub fn get_resource(&self, resource_entry: &ResourceEntry) -> Option<&ResourceState> {
        let key = Self::resource_key(&resource_entry.source, &resource_entry.target_dir);
        self.resource_index
            .get(&key)
            .map(|&index| &self.resources[index])
    }

    pub fn add_resource(&mut self, resource_state: ResourceState) {
        let key = Self::resource_key(&resource_state.source, &resource_state.target_dir);
        let index = self.resources.len();
        self.resources.push(resource_state);
        self.resource_index.insert(key, index);
    }

    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("Failed to read installer state at {}", path.display()))?;
        let mut state: InstallerState =
            serde_json::from_str(&raw).context("Failed to deserialize installer-state.json")?;
        // Build indexes
        debug_assert!(state.mod_index.is_empty());
        state.mod_index.clear();
        for (i, mod_state) in state.mods.iter().enumerate() {
            let key = Self::mod_key(&mod_state.source);
            state.mod_index.insert(key, i);
        }
        debug_assert!(state.resource_index.is_empty());
        state.resource_index.clear();
        for (i, resource_state) in state.resources.iter().enumerate() {
            let key = Self::resource_key(&resource_state.source, &resource_state.target_dir);
            state.resource_index.insert(key, i);
        }

        Ok(state)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize installer state to JSON")?;
        fs::write(path, json)
            .with_context(|| format!("Failed to write installer state to {}", path.display()))?;
        Ok(())
    }

    pub fn finalize(&mut self, path: &Path) -> Result<()> {
        self.process_mode = None;
        self.save(path)?;
        Ok(())
    }

    fn migrate_pack_version() -> Version {
        log::warn!("packVersion is missing in installer state, defaulting to '0.0.0'");
        Version::new(0, 0, 0)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModLoaderState {
    pub file_name: String,
    pub url: String,
    pub hash: String,
}

impl ModLoaderState {
    pub fn equals(&self, config: &ModLoader) -> bool {
        self.url == config.url && self.hash == config.hash
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModState {
    pub file_name: String,
    #[serde(flatten)]
    pub source: SourceType,
    pub hash: String,
}

impl ModState {
    pub fn equals(&self, config: &ModEntry, is_ignore_hash: bool) -> bool {
        self.source == config.source && (is_ignore_hash || self.hash == config.hash)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceState {
    pub file_name: String,
    #[serde(flatten)]
    pub source: SourceType,
    pub hash: String,
    pub target_dir: String,
    pub decompress: bool,
}

impl ResourceState {
    pub fn equals(&self, config: &ResourceEntry) -> bool {
        self.source == config.source && self.hash == config.hash
    }
}
