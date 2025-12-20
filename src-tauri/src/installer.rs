use std::{
    cmp::Ordering,
    env,
    fmt::{self, Display},
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{anyhow, bail, Context, Result};
use semver::Version;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use zip::ZipArchive;

use crate::config::{ModPackConfig, ResourceEntry, Side};
use crate::downloader::{DownloadManager, DownloadProgress};
use crate::launcher::{LauncherProfile, LauncherProfiles};
use crate::state::{InstallerState, ModLoaderState, ModState, ResourceState};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallerMode {
    Install,
    Update,
}

impl Display for InstallerMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallerMode::Install => write!(f, "Install"),
            InstallerMode::Update => write!(f, "Update"),
        }
    }
}

pub struct Installer {
    mode: InstallerMode,
    app: AppHandle,
    download_manager: DownloadManager,
    config: ModPackConfig,
    install_dir: PathBuf,
    side: Side,
    temp_dir: PathBuf,
    state_path: PathBuf,
}

impl Installer {
    pub fn new(
        mode: InstallerMode,
        app: AppHandle,
        config_path: PathBuf,
        install_dir: PathBuf,
        side: Side,
        app_dir: PathBuf,
        state_path: PathBuf,
    ) -> Result<Self> {
        assert_ne!(&side, &Side::Both);
        Ok(Self {
            mode,
            app,
            download_manager: DownloadManager::new()?,
            config: ModPackConfig::load_from_path(&config_path)?,
            install_dir: install_dir.clone(),
            side,
            temp_dir: app_dir.join(".temp"),
            state_path,
        })
    }

    pub fn can_install(config_path: &Path, state_path: &Path) -> Result<()> {
        if !config_path.exists() {
            bail!("Config file is not found.");
        }
        if !state_path.exists() {
            return Ok(());
        }
        let state = InstallerState::load(&state_path)?;
        Self::can_install_state(&state)
    }

    fn can_install_state(state: &InstallerState) -> Result<()> {
        match state.get_process_mode() {
            Some(mode) if mode != InstallerMode::Install => {
                bail!("Another mode ({:?}) is already in progress.", mode)
            }
            _ => Ok(()),
        }
    }

    pub fn can_update(config_path: &Path, state_path: &Path) -> Result<()> {
        let config = ModPackConfig::load_from_path(&config_path)?;
        Self::can_update_state(&config, &state_path).map(|_| ())
    }

    fn can_update_state(config: &ModPackConfig, state_path: &Path) -> Result<InstallerState> {
        if !state_path.exists() {
            bail!("Installer state file is not found.");
        }
        let state = InstallerState::load(&state_path)?;
        match state.get_process_mode() {
            None => {
                if config.get_pack_version() > state.get_pack_version() {
                    Ok(state)
                } else {
                    bail!("No update is needed.");
                }
            }
            Some(mode) if mode != InstallerMode::Update => {
                bail!("Another mode ({:?}) is already in progress.", mode)
            }
            Some(_) => Ok(state),
        }
    }

    pub async fn run(mut self) -> Result<()> {
        self.emit_progress(0.);
        match self.mode {
            InstallerMode::Install => self.run_install().await,
            InstallerMode::Update => self.run_update().await,
        }
    }

    async fn run_install(&mut self) -> Result<()> {
        log::info!("Starting installation...");
        self.prepare_temp_dir()?;
        let installer_version = self.app.package_info().version.clone();
        let mut is_retry = false;
        let mut state = if !self.state_path.exists() {
            InstallerState::new(&installer_version, &self.config.get_pack_version())
        } else {
            let s = InstallerState::load(&self.state_path)?;
            Self::can_install_state(&s)?;
            is_retry = s.get_process_mode().is_none();
            if !is_retry {
                log::info!("Resuming previous installation process...");
            } else {
                log::info!("Retry adding profile and launching mod loader...");
            }
            s
        };
        state.set_process_mode(self.mode);
        state.save(&self.state_path)?;
        if !is_retry {
            let total_steps = self.total_download_steps(self.mode, &state);
            let mut completed_steps = 0u32;
            // Download Mod loader
            self.emit_change_phase(Phase::DownloadModLoader);
            let loader_config = &self.config.get_mod_loader();
            if let Some(downloaded_loader) = state.get_mod_loader() {
                if !downloaded_loader.equals(loader_config) {
                    log::error!(
                        "Mod loader {} is downloaded, but uploaded file was changed. Skipping.",
                        loader_config.name
                    );
                } else {
                    log::info!(
                        "Mod loader {} is already downloaded, skipping download.",
                        loader_config.name
                    );
                }
            } else {
                let file_name = self
                    .ensure_download(
                        &loader_config.url,
                        &loader_config.name,
                        &loader_config.hash,
                        &self.install_dir,
                        false,
                        completed_steps,
                        total_steps,
                    )
                    .await?;
                state.set_mod_loader(ModLoaderState {
                    file_name,
                    url: loader_config.url.clone(),
                    hash: loader_config.hash.clone(),
                });
                state.save(&self.state_path)?;
            }
            completed_steps += 1u32;
            self.emit_progress(completed_steps as f32 / total_steps as f32);
            // Mods
            self.emit_change_phase(Phase::DownloadMods);
            self.download_mods(&mut state, &mut completed_steps, total_steps)
                .await?;
            // Resources
            self.emit_change_phase(Phase::DownloadResources);
            self.download_resources(&mut state, &mut completed_steps, total_steps)
                .await?;
            debug_assert_eq!(completed_steps, total_steps);
        }
        self.emit_progress(1.);
        // Add profile to launcher
        self.emit_change_phase(Phase::AddProfile);
        if let Err(e) = self.add_launcher_profile() {
            log::warn!("Failed to add launcher profile: {e:?}");
            self.emit_add_alert(AlertLevel::Warning, "alertOnFailedAddProfile");
        }
        // Auto-open mod loader if configured
        if self.config.get_mod_loader().auto_open {
            self.emit_change_phase(Phase::LaunchModLoader);
            if let Err(e) = self.launch_mod_loader() {
                log::warn!("Failed to launch mod loader: {e:?}");
                self.emit_add_alert(AlertLevel::Warning, "alertOnFailedLaunchModLoader");
            }
        }
        state.set_installer_version(&installer_version);
        state.finalize(&self.state_path)?;
        log::info!("Installation completed.");

        Ok(())
    }

    async fn run_update(&mut self) -> Result<()> {
        log::info!("Starting update...");
        self.prepare_temp_dir()?;
        let mut state = Self::can_update_state(&self.config, &self.state_path)?;
        state.set_process_mode(self.mode);
        state.save(&self.state_path)?;
        let total_steps = self.total_download_steps(self.mode, &state);
        let mut completed_steps = 0u32;
        // Remove mods
        self.emit_change_phase(Phase::RemoveMods);
        let mods_dir = self.get_mods_dir();
        let all_mods: Vec<ModState> = state.get_all_mods().into_iter().cloned().collect();
        for mod_state in all_mods {
            if !self.config.has_mod(&mod_state.source) {
                let mod_path = mods_dir.join(&mod_state.file_name);
                if mod_path.exists() {
                    log::info!("Removing mod: {}", mod_state.file_name);
                    fs::remove_file(&mod_path).with_context(|| {
                        format!("Failed to remove mod file: {}", mod_path.display())
                    })?;
                } else {
                    log::warn!("Mod file to remove does not exist: {}", mod_path.display());
                }
                state.remove_mod(&mod_state);
                state.save(&self.state_path)?;
            }
            completed_steps += 1u32;
            self.emit_progress(completed_steps as f32 / total_steps as f32);
        }
        // Add mods
        self.emit_change_phase(Phase::DownloadMods);
        self.download_mods(&mut state, &mut completed_steps, total_steps)
            .await?;
        // Add resources
        self.emit_change_phase(Phase::DownloadResources);
        self.download_resources(&mut state, &mut completed_steps, total_steps)
            .await?;
        // Update settings
        self.emit_change_phase(Phase::UpdateSettings);
        self.update_settings(&mut state, &mut completed_steps, total_steps)
            .await?;
        debug_assert_eq!(completed_steps, total_steps);
        self.emit_progress(1.);
        state.set_installer_version(&self.app.package_info().version);
        state.set_pack_version(&self.config.get_pack_version());
        state.finalize(&self.state_path)?;
        log::info!("Update completed.");

        Ok(())
    }

    fn prepare_temp_dir(&self) -> Result<()> {
        if self.temp_dir.exists() {
            fs::remove_dir_all(&self.temp_dir).with_context(|| {
                format!("Failed to wipe temp directory {}", self.temp_dir.display())
            })?;
        }
        fs::create_dir_all(&self.temp_dir).with_context(|| {
            format!(
                "Failed to create temp directory {}",
                self.temp_dir.display()
            )
        })?;
        Ok(())
    }

    fn get_update_settings_steps(now: &Version, new: &Version) -> u32 {
        let mut steps = 0u32;
        let v1_2_0 = Version::parse("1.2.0").unwrap();
        if now < &v1_2_0 && new >= &v1_2_0 {
            // v1.2.0 update
            steps += 1; // Update configs
        }
        let v1_2_1 = Version::parse("1.2.1").unwrap();
        if now < &v1_2_1 && new >= &v1_2_1 {
            // v1.2.1 update
            steps += 1; // Update configs
        }
        let v1_3_0 = Version::parse("1.3.0").unwrap();
        if now < &v1_3_0 && new >= &v1_3_0 {
            // v1.3.0 update
            steps += 13; // Update configs
        }
        steps
    }

    fn total_download_steps(&self, mode: InstallerMode, state: &InstallerState) -> u32 {
        let mut steps = 0u32;
        if mode == InstallerMode::Install {
            steps += 1; // Mod Loader
        }
        if mode == InstallerMode::Update {
            steps += state.get_mod_count() as u32;
        }
        steps += self
            .config
            .get_mods()
            .iter()
            .filter(|mod_entry| mod_entry.should_install(&self.side))
            .count() as u32;
        steps += self
            .config
            .get_resources()
            .iter()
            .filter(|resource_entry| resource_entry.should_install(&self.side))
            .count() as u32;
        if mode == InstallerMode::Update {
            steps += Self::get_update_settings_steps(
                &state.get_pack_version(),
                &self.config.get_pack_version(),
            );
        }
        steps
    }

    async fn ensure_download(
        &self,
        url: &str,
        name: &str,
        expected_hash: &str,
        final_dir: &Path,
        is_decompress: bool,
        completed_steps: u32,
        total_steps: u32,
    ) -> Result<String> {
        log::info!("Downloading {name} from {url} ...");
        self.emit_change_detail(name);
        let outcome = self
            .download_manager
            .download_to_dir(
                url,
                &self.temp_dir,
                Some(move |progress: DownloadProgress| -> Result<()> {
                    if progress.total_bytes.is_none() {
                        return Ok(());
                    }
                    let total = progress.total_bytes.unwrap();
                    let fraction = if total != 0 {
                        progress.received_bytes as f32 / total as f32
                    } else {
                        0.0
                    };
                    self.emit_progress((completed_steps as f32 + fraction) / total_steps as f32);
                    Ok(())
                }),
            )
            .await?;
        let file_name = outcome
            .path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Could not extract file name from downloaded file"))?;
        verify_hash(expected_hash, &outcome.hash, &outcome.path)?;
        if !is_decompress {
            let final_path = final_dir.join(&file_name);
            move_file(&outcome.path, &final_path)?;
            log::info!("Downloaded {name}.");
        } else {
            log::info!("Extracting {name} to {} ...", final_dir.display());
            extract_zip(&outcome.path, final_dir)?;
            if let Err(e) = fs::remove_file(&outcome.path) {
                log::warn!(
                    "Failed to remove temporary file {}: {e:?}",
                    outcome.path.display()
                );
            }
            log::info!("Extracted {name}.");
        }

        self.emit_progress((completed_steps + 1) as f32 / total_steps as f32);

        Ok(file_name.to_string_lossy().to_string())
    }

    fn get_mods_dir(&self) -> PathBuf {
        self.install_dir.join("mods")
    }

    fn get_resource_dir(&self, entry: &ResourceEntry) -> PathBuf {
        self.install_dir.join(&entry.target_dir)
    }

    async fn download_mods(
        &self,
        state: &mut InstallerState,
        completed_steps: &mut u32,
        total_steps: u32,
    ) -> Result<()> {
        let mods_dir = self.get_mods_dir();
        for mod_entry in self.config.get_mods() {
            if !mod_entry.should_install(&self.side) {
                continue;
            }
            let needs_download = state.get_mod(mod_entry).map_or(true, |downloaded_mod| {
                if !downloaded_mod.equals(mod_entry, false) {
                    log::warn!(
                        "Mod {} is downloaded, but uploaded file was changed.",
                        mod_entry.name
                    );
                    true
                } else {
                    log::info!(
                        "Mod {} is already downloaded, skipping download.",
                        mod_entry.name
                    );
                    false
                }
            });
            if needs_download {
                let url = mod_entry.source.get_download_url().await?;
                let file_name = self
                    .ensure_download(
                        &url,
                        &mod_entry.name,
                        &mod_entry.hash,
                        &mods_dir,
                        false,
                        *completed_steps,
                        total_steps,
                    )
                    .await?;
                state.add_mod(ModState {
                    file_name,
                    source: mod_entry.source.clone(),
                    hash: mod_entry.hash.clone(),
                });
                state.save(&self.state_path)?;
            }
            *completed_steps += 1u32;
            self.emit_progress(*completed_steps as f32 / total_steps as f32);
        }
        Ok(())
    }

    async fn download_resources(
        &self,
        state: &mut InstallerState,
        completed_steps: &mut u32,
        total_steps: u32,
    ) -> Result<()> {
        for resource_entry in self.config.get_resources() {
            if !resource_entry.should_install(&self.side) {
                continue;
            }
            let needs_download =
                state
                    .get_resource(resource_entry)
                    .map_or(true, |downloaded_resource| {
                        if !downloaded_resource.equals(resource_entry) {
                            log::warn!(
                                "Resource {} is downloaded, but uploaded file was changed.",
                                resource_entry.name
                            );
                            true
                        } else {
                            log::info!(
                                "Resource {} is already downloaded, skipping download.",
                                resource_entry.name
                            );
                            false
                        }
                    });
            if needs_download {
                let url = resource_entry.source.get_download_url().await?;
                let target_dir = self.get_resource_dir(resource_entry);
                let file_name = self
                    .ensure_download(
                        &url,
                        &resource_entry.name,
                        &resource_entry.hash,
                        &target_dir,
                        resource_entry.decompress,
                        *completed_steps,
                        total_steps,
                    )
                    .await?;
                state.add_resource(ResourceState {
                    file_name,
                    source: resource_entry.source.clone(),
                    hash: resource_entry.hash.clone(),
                    target_dir: resource_entry.target_dir.clone(),
                    decompress: resource_entry.decompress,
                });
                state.save(&self.state_path)?;
            }
            *completed_steps += 1u32;
            self.emit_progress(*completed_steps as f32 / total_steps as f32);
        }
        Ok(())
    }

    async fn update_settings(
        &self,
        state: &mut InstallerState,
        completed_steps: &mut u32,
        total_steps: u32,
    ) -> Result<()> {
        let now = state.get_pack_version();
        let new = self.config.get_pack_version();
        let v1_2_0 = Version::parse("1.2.0").unwrap();
        if now < &v1_2_0 && new >= &v1_2_0 {
            // v1.2.0 update
            log::info!("Updating config files for v1.2.0...");
            let url = "https://github.com/kyazuki/Makibania-Modpack-Resources/releases/download/v1.2.0/configs.zip";
            self.ensure_download(
                url,
                "configs",
                "4cb14e94845a0f03775c0d1b8f3f0cbddb675ddb",
                &self.install_dir.join("config"),
                true,
                *completed_steps,
                total_steps,
            )
            .await?;
            *completed_steps += 1u32;
            self.emit_progress(*completed_steps as f32 / total_steps as f32);
        }
        let v1_2_1 = Version::parse("1.2.1").unwrap();
        if now < &v1_2_1 && new >= &v1_2_1 {
            // v1.2.1 update
            log::info!("Updating config files for v1.2.1...");
            let url = "https://github.com/kyazuki/Makibania-Modpack-Resources/releases/download/v1.2.1/configs.zip";
            self.ensure_download(
                url,
                "configs",
                "9e5f63a8b1a6da42792ffc1563dcd6c6f6eac495",
                &self.install_dir.join("config"),
                true,
                *completed_steps,
                total_steps,
            )
            .await?;
            *completed_steps += 1u32;
            self.emit_progress(*completed_steps as f32 / total_steps as f32)
        }
        let v1_3_0 = Version::parse("1.3.0").unwrap();
        if now < &v1_3_0 && new >= &v1_3_0 {
            // v1.3.0 update
            log::info!("Updating config files for v1.3.0...");
            for path in &[
                PathBuf::from("fancymenu/customization/loading_makibania_default.txt"),
                PathBuf::from("fancymenu/customization/options_makibania.txt"),
                PathBuf::from("fancymenu/customization/title_makibania_default.txt"),
                PathBuf::from("fancymenu/customization/universal_makibania_bg.txt"),
                PathBuf::from("fancymenu/custom_gui_screens.txt"),
                PathBuf::from("fancymenu/customizablemenus.txt"),
                PathBuf::from("fancymenu/options.txt"),
                PathBuf::from("fancymenu/user_variables.db"),
                PathBuf::from("ftbquests/quests/chapters/welcome.snbt"),
                PathBuf::from("ftbquests/quests/lang/en_us.snbt"),
                PathBuf::from("ftbquests/quests/lang/ja_jp.snbt"),
                PathBuf::from("ftbquests/quests/chapter_groups.snbt"),
                PathBuf::from("ftbquests/quests/data.snbt"),
            ] {
                self.overwrite_config(path).await?;
                *completed_steps += 1u32;
                self.emit_progress(*completed_steps as f32 / total_steps as f32)
            }
        }

        Ok(())
    }

    async fn overwrite_config(&self, path: &Path) -> Result<()> {
        log::info!("Overwriting config file: {}", path.display());
        if !path.is_relative() {
            log::error!("Config path must be relative: {}", path.display());
            bail!("Config path must be relative: {}", path.display());
        }
        let original_path = self
            .install_dir
            .join("configureddefaults/config")
            .join(path);
        let target_path = self.install_dir.join("config").join(path);
        if !original_path.exists() {
            log::error!(
                "Original config file does not exist: {}",
                original_path.display()
            );
            bail!(
                "Original config file does not exist: {}",
                original_path.display()
            );
        }
        fs::copy(&original_path, &target_path)?;
        log::info!("Overwrote: {}", target_path.display());

        Ok(())
    }

    fn emit_change_phase(&self, phase: Phase) {
        debug_assert!(phase != Phase::DownloadModLoader || self.mode == InstallerMode::Install);
        debug_assert!(phase != Phase::RemoveMods || self.mode == InstallerMode::Update);
        debug_assert!(phase != Phase::UpdateSettings || self.mode == InstallerMode::Update);
        debug_assert!(phase != Phase::AddProfile || self.mode == InstallerMode::Install);
        debug_assert!(phase != Phase::LaunchModLoader || self.mode == InstallerMode::Install);
        emit_event(
            &self.app,
            InstallerEvent::ChangePhase(ChangePhasePayload { phase: phase }),
        );
    }

    fn emit_change_detail(&self, detail: &str) {
        emit_event(
            &self.app,
            InstallerEvent::ChangeDetail(ChangeDetailPayload {
                detail: detail.to_string(),
            }),
        );
    }

    fn emit_progress(&self, progress: f32) {
        emit_event(
            &self.app,
            InstallerEvent::UpdateProgress(UpdateProgressPayload { progress }),
        );
    }

    fn emit_add_alert(&self, level: AlertLevel, translation_key: &str) {
        emit_event(
            &self.app,
            InstallerEvent::AddAlert(AddAlertPayload {
                level,
                translation_key: translation_key.to_string(),
            }),
        );
    }

    fn add_launcher_profile(&self) -> Result<()> {
        log::info!("Adding launcher profile...");
        let profiles_path = if cfg!(target_os = "windows") {
            let appdata = env::var("APPDATA").context("APPDATA environment variable not found")?;
            PathBuf::from(appdata)
                .join(".minecraft")
                .join("launcher_profiles.json")
        } else if cfg!(target_os = "macos") {
            let home = env::var("HOME").context("HOME environment variable not found")?;
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("minecraft")
                .join("launcher_profiles.json")
        } else {
            bail!("Unsupported operating system: {}", env::consts::OS);
        };
        if !profiles_path.exists() {
            bail!("Launcher profiles file not found. ");
        }
        // Load existing profiles
        let content =
            fs::read_to_string(&profiles_path).context("Failed to read launcher_profiles.json")?;
        let mut launcher_profiles: LauncherProfiles =
            serde_json::from_str(&content).context("Failed to parse launcher_profiles.json")?;
        // Check if profile already exists
        for profile in launcher_profiles.profiles.values() {
            if profile.name == self.config.get_profile().name {
                log::info!(
                    "Launcher profile '{}' already exists, skipping addition.",
                    profile.name
                );
                return Ok(());
            }
        }
        // Insert new profile
        let profile_id = uuid::Uuid::new_v4().simple().to_string();
        let now = chrono::Utc::now();
        let now_rounded =
            chrono::DateTime::from_timestamp_millis(now.timestamp_millis()).unwrap_or(now);
        let new_profile = LauncherProfile {
            created: Some(now_rounded),
            game_dir: Some(self.install_dir.clone()),
            icon: self.config.get_profile().icon.clone(),
            java_args: self.config.get_profile().jvm_args.clone(),
            java_dir: None,
            last_used: Some(now_rounded),
            last_version_id: self.config.get_profile().version.clone(),
            name: self.config.get_profile().name.clone(),
            resolution: None,
            skip_jre_version_check: None,
            profile_type: "custom".to_string(),
        };
        if launcher_profiles
            .profiles
            .insert(profile_id.clone(), new_profile)
            .is_some()
        {
            bail!("Profile ID '{profile_id}' already exists in launcher profiles");
        }
        // Backup original file
        let mut backup_path = profiles_path.with_extension("json.bak");
        let mut backup_index = 1;
        while backup_path.exists() {
            backup_path = profiles_path.with_extension(format!("json.bak{backup_index}"));
            backup_index += 1;
        }
        fs::rename(&profiles_path, &backup_path)
            .context("Failed to backup launcher_profiles.json")?;
        log::info!(
            "Backed up launcher_profiles.json to {}",
            backup_path.display()
        );
        // Save profiles
        let profiles_json = serde_json::to_string_pretty(&launcher_profiles)
            .context("Failed to serialize profiles")?;
        fs::write(&profiles_path, profiles_json)
            .context("Failed to write launcher_profiles.json")?;
        log::info!(
            "Added profile '{}' to launcher.",
            self.config.get_profile().name
        );

        Ok(())
    }

    fn launch_mod_loader(&self) -> Result<()> {
        log::info!("Launching mod loader...");
        // Find mod loader jar file
        let jar_files: Vec<_> = fs::read_dir(&self.install_dir)
            .context("Failed to read install directory")?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("jar"))
                    .unwrap_or(false)
            })
            .collect();
        if jar_files.is_empty() {
            bail!("Mod loader installer JAR file not found.");
        }
        let jar_path = &jar_files.first().unwrap().path();
        log::info!("Found mod loader: {}", jar_path.display());
        // Find Java executable
        let java_exe = find_java().ok_or_else(|| anyhow!("Java executable not found"))?;
        log::info!("Using Java: {}", java_exe.display());
        // Launch jar file
        let mut command = Command::new(java_exe);
        command
            .arg("-jar")
            .arg(jar_path)
            .current_dir(&self.install_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(winapi::um::winbase::CREATE_NO_WINDOW);
        }
        command
            .spawn()
            .context("Failed to launch mod loader installer")?;
        self.emit_add_alert(AlertLevel::Info, "alertOnLaunchModLoader");
        log::info!("Launched mod loader installer.");

        Ok(())
    }
}

fn find_java() -> Option<PathBuf> {
    // 1. Check system java command
    log::info!("Searching for system java...");
    match Command::new(if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    })
    .arg("java")
    .output()
    {
        Ok(output) => {
            if output.status.success() {
                if let Ok(path_str) = String::from_utf8(output.stdout) {
                    let path = PathBuf::from(path_str.trim());
                    if path.exists() {
                        return Some(path);
                    }
                }
            }
        }
        Err(error) => {
            log::warn!("Failed to find system java: {error:?}");
        }
    }
    // 2. Check Minecraft Launcher App runtime
    log::info!("Searching for java from minecraft...");
    if cfg!(target_os = "windows") {
        match env::var("LOCALAPPDATA") {
            Ok(local_appdata) => {
                let runtimes_dir = PathBuf::from(local_appdata)
                    .join("Packages")
                    .join("Microsoft.4297127D64EC6_8wekyb3d8bbwe")
                    .join("LocalCache")
                    .join("Local")
                    .join("runtime");
                if let Some(java) = search_runtime_dir(&runtimes_dir) {
                    return Some(java);
                }
            }
            Err(error) => {
                log::warn!("LOCALAPPDATA environment variable not found: {error:?}");
            }
        }
    } else if cfg!(target_os = "macos") {
        match env::var("HOME") {
            Ok(home) => {
                let runtimes_dir = PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("minecraft")
                    .join("runtime");
                if let Some(java) = search_runtime_dir(&runtimes_dir) {
                    return Some(java);
                }
            }
            Err(error) => {
                log::warn!("HOME environment variable not found: {error:?}");
            }
        }
    }

    None
}

fn search_runtime_dir(runtime_dir: &Path) -> Option<PathBuf> {
    if !runtime_dir.exists() {
        return None;
    }
    let mut dirs: Vec<_> = fs::read_dir(runtime_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    dirs.sort_by(|a, b| {
        let name_a = a.file_name();
        let name_b = b.file_name();
        let a_is_newer_java = name_a.to_string_lossy().starts_with("java-runtime-");
        let b_is_newer_java = name_b.to_string_lossy().starts_with("java-runtime-");
        match (a_is_newer_java, b_is_newer_java) {
            (true, false) => Ordering::Less,    // java-runtime-* comes first
            (false, true) => Ordering::Greater, // jre-* comes later
            _ => name_b.cmp(&name_a),           // Among same type, reverse order (newer first)
        }
    });
    for entry in dirs {
        let path = entry.path();
        let java_exe = if cfg!(target_os = "windows") {
            path.join("bin").join("javaw.exe")
        } else {
            path.join("bin").join("java")
        };
        if java_exe.exists() {
            return Some(java_exe);
        }
    }

    None
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum InstallerEvent {
    ChangePhase(ChangePhasePayload),
    ChangeDetail(ChangeDetailPayload),
    UpdateProgress(UpdateProgressPayload),
    AddAlert(AddAlertPayload),
}

#[derive(Clone, Debug, Serialize)]
struct ChangePhasePayload {
    phase: Phase,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Phase {
    DownloadModLoader,
    RemoveMods,
    DownloadMods,
    DownloadResources,
    UpdateSettings,
    AddProfile,
    LaunchModLoader,
}

#[derive(Clone, Debug, Serialize)]
struct ChangeDetailPayload {
    detail: String,
}

#[derive(Clone, Debug, Serialize)]
struct UpdateProgressPayload {
    progress: f32,
}

#[derive(Clone, Debug, Serialize)]
struct AddAlertPayload {
    level: AlertLevel,
    translation_key: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum AlertLevel {
    Info,
    Warning,
}

fn hash_matches(expected: &str, actual: &str) -> bool {
    expected.eq_ignore_ascii_case(actual)
}

fn verify_hash(expected: &str, actual: &str, final_path: &Path) -> Result<()> {
    if hash_matches(expected, actual) {
        Ok(())
    } else {
        bail!(
            "Hash mismatch for {}. Expected {expected}, got {actual}",
            final_path.display()
        );
    }
}

fn emit_event(app: &AppHandle, payload: InstallerEvent) {
    if let Err(e) = app.emit("installer://event", &payload) {
        log::warn!("Failed to emit installer event. payload: {payload:?}, error: {e:?}");
    }
}

fn move_file(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create destination directory: {}",
                parent.display()
            )
        })?;
    }
    if destination.exists() {
        log::warn!(
            "Destination file ({}) already exists and will be overwritten.",
            destination.display()
        );
    }
    fs::rename(source, destination)?;

    Ok(())
}

fn extract_zip(zip_path: &Path, target_dir: &Path) -> Result<()> {
    let file = File::open(zip_path)?;
    ZipArchive::new(file)?.extract(target_dir)?;

    Ok(())
}
