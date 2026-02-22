use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEFAULT_BROKER_URL: &str = "mqtt://localhost:1883";
// On macOS/Linux via Wine, VFP can only reliably handle 1 concurrent instance.
// On Windows (native), 4+ concurrent processes work fine.
#[cfg(target_os = "windows")]
const DEFAULT_MAX_CONCURRENT: usize = 4;
#[cfg(not(target_os = "windows"))]
const DEFAULT_MAX_CONCURRENT: usize = 1;
const DEFAULT_TDS_VERSION: &str = "TDS6307";
const DEFAULT_DOWNLOAD_URL: &str =
    "https://www.tcmc.or.th/_content_images/download/fileupload/S0021.zip";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub broker_url: String,
    pub client_id: String,
    pub exe_base_path: String,
    pub max_concurrent: usize,
    pub version: String,
    pub auto_start: bool,
    pub download_url: String,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            broker_url: DEFAULT_BROKER_URL.to_string(),
            client_id: format!("casemind-worker-{}", uuid::Uuid::new_v4()),
            exe_base_path: String::new(),
            max_concurrent: DEFAULT_MAX_CONCURRENT,
            version: DEFAULT_TDS_VERSION.to_string(),
            auto_start: false,
            download_url: DEFAULT_DOWNLOAD_URL.to_string(),
        }
    }
}

impl WorkerConfig {
    pub fn config_dir() -> PathBuf {
        let proj_dirs = directories::ProjectDirs::from("com", "aegisx", "casemind-worker")
            .expect("Failed to get project directories");
        proj_dirs.config_dir().to_path_buf()
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => match serde_json::from_str(&contents) {
                    Ok(config) => return config,
                    Err(e) => {
                        log::warn!("Failed to parse config: {}, using defaults", e);
                    }
                },
                Err(e) => {
                    log::warn!("Failed to read config: {}, using defaults", e);
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn exe_path(&self) -> PathBuf {
        PathBuf::from(&self.exe_base_path).join("TGrp6305.exe")
    }

    pub fn has_valid_exe(&self) -> bool {
        !self.exe_base_path.is_empty() && self.exe_path().exists()
    }
}
