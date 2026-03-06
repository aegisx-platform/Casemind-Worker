use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEFAULT_BROKER_HOST: &str = "localhost";
const DEFAULT_BROKER_PORT: u16 = 1883;
// On macOS/Linux via Wine, VFP can only reliably handle 1 concurrent instance.
// On Windows (native), 4+ concurrent processes work fine.
#[cfg(target_os = "windows")]
const DEFAULT_MAX_CONCURRENT: usize = 4;
#[cfg(not(target_os = "windows"))]
const DEFAULT_MAX_CONCURRENT: usize = 1;
const DEFAULT_TDS_VERSION: &str = "TDS6307";
const DEFAULT_DOWNLOAD_URL: &str =
    "https://www.tcmc.or.th/_content_images/download/fileupload/S0021.zip";
const DEFAULT_KEEP_ALIVE_SECS: u32 = 30;

const DEFAULT_TOPIC_PENDING: &str = "tasks/drg/pending";
const DEFAULT_TOPIC_RESULTS: &str = "tasks/drg/results";
const DEFAULT_TOPIC_HEALTH: &str = "workers/health";
const DEFAULT_TOPIC_REGISTER: &str = "workers/register";

fn default_broker_host() -> String {
    DEFAULT_BROKER_HOST.to_string()
}
fn default_broker_port() -> u16 {
    DEFAULT_BROKER_PORT
}
fn default_keep_alive_secs() -> u32 {
    DEFAULT_KEEP_ALIVE_SECS
}
fn default_topic_pending() -> String {
    DEFAULT_TOPIC_PENDING.to_string()
}
fn default_topic_results() -> String {
    DEFAULT_TOPIC_RESULTS.to_string()
}
fn default_topic_health() -> String {
    DEFAULT_TOPIC_HEALTH.to_string()
}
fn default_topic_register() -> String {
    DEFAULT_TOPIC_REGISTER.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Legacy field — only used when reading old config files that have broker_url.
    /// Skipped during serialization so new configs use broker_host/broker_port.
    #[serde(default, skip_serializing)]
    broker_url: Option<String>,

    #[serde(default = "default_broker_host")]
    pub broker_host: String,
    #[serde(default = "default_broker_port")]
    pub broker_port: u16,
    pub client_id: String,
    pub exe_base_path: String,
    pub max_concurrent: usize,
    pub version: String,
    pub auto_start: bool,
    pub download_url: String,

    // MQTT Authentication
    #[serde(default)]
    pub mqtt_username: Option<String>,
    #[serde(default)]
    pub mqtt_password: Option<String>,

    // TLS/SSL
    #[serde(default)]
    pub use_tls: bool,
    #[serde(default)]
    pub tls_ca_cert_path: Option<String>,

    // Advanced
    #[serde(default = "default_keep_alive_secs")]
    pub keep_alive_secs: u32,

    // MQTT Topics
    #[serde(default = "default_topic_pending")]
    pub topic_pending: String,
    #[serde(default = "default_topic_results")]
    pub topic_results: String,
    #[serde(default = "default_topic_health")]
    pub topic_health: String,
    #[serde(default = "default_topic_register")]
    pub topic_register: String,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            broker_url: None,
            broker_host: DEFAULT_BROKER_HOST.to_string(),
            broker_port: DEFAULT_BROKER_PORT,
            client_id: format!("casemind-worker-{}", uuid::Uuid::new_v4()),
            exe_base_path: String::new(),
            max_concurrent: DEFAULT_MAX_CONCURRENT,
            version: DEFAULT_TDS_VERSION.to_string(),
            auto_start: false,
            download_url: DEFAULT_DOWNLOAD_URL.to_string(),
            mqtt_username: None,
            mqtt_password: None,
            use_tls: false,
            tls_ca_cert_path: None,
            keep_alive_secs: DEFAULT_KEEP_ALIVE_SECS,
            topic_pending: DEFAULT_TOPIC_PENDING.to_string(),
            topic_results: DEFAULT_TOPIC_RESULTS.to_string(),
            topic_health: DEFAULT_TOPIC_HEALTH.to_string(),
            topic_register: DEFAULT_TOPIC_REGISTER.to_string(),
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
                Ok(contents) => match serde_json::from_str::<WorkerConfig>(&contents) {
                    Ok(mut config) => {
                        // Migrate from legacy broker_url format (e.g. "mqtt://host:port")
                        if let Some(url) = config.broker_url.take() {
                            if config.broker_host == DEFAULT_BROKER_HOST
                                && config.broker_port == DEFAULT_BROKER_PORT
                            {
                                let stripped = url
                                    .strip_prefix("mqtts://")
                                    .or_else(|| url.strip_prefix("mqtt://"))
                                    .unwrap_or(&url);
                                let parts: Vec<&str> = stripped.split(':').collect();
                                if let Some(&host) = parts.first() {
                                    if !host.is_empty() {
                                        config.broker_host = host.to_string();
                                    }
                                }
                                if let Some(port) = parts.get(1).and_then(|p| p.parse().ok()) {
                                    config.broker_port = port;
                                }
                                if url.starts_with("mqtts://") {
                                    config.use_tls = true;
                                }
                                log::info!(
                                    "Migrated legacy broker_url to {}:{}",
                                    config.broker_host,
                                    config.broker_port
                                );
                            }
                        }
                        return config;
                    }
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

    /// Construct a display-friendly broker URL string (e.g. "mqtt://host:port").
    pub fn broker_display_url(&self) -> String {
        let scheme = if self.use_tls { "mqtts" } else { "mqtt" };
        format!("{}://{}:{}", scheme, self.broker_host, self.broker_port)
    }
}
