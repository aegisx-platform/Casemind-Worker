use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS, Transport};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::config::WorkerConfig;

/// MQTT topics
// Note: $share/group/ prefix requires MQTT v5; use plain topic for v3.1.1 compatibility
const TOPIC_PENDING: &str = "tasks/drg/pending";
const TOPIC_RESULTS_PREFIX: &str = "tasks/drg/results/";
const TOPIC_HEALTH_PREFIX: &str = "workers/health/";
const TOPIC_REGISTER_PREFIX: &str = "workers/register/";

/// Task message published by CaseMind API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrgTask {
    pub request_id: String,
    pub version_id: String,
    pub cases: Vec<DrgTaskCase>,
    pub published_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrgTaskCase {
    pub pdx: String,
    pub sdx: Vec<String>,
    pub procedures: Vec<String>,
    pub age: i32,
    pub age_in_days: Option<i32>,
    pub sex: String,
    pub discharge_type: i32,
    pub los: i32,
    pub admission_weight: Option<f64>,
}

/// Result message published by worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrgResult {
    pub request_id: String,
    pub worker_id: String,
    pub version_id: String,
    pub cases: Vec<DrgResultCase>,
    pub processing_time_ms: u64,
    pub completed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrgResultCase {
    pub drg: String,
    pub mdc: String,
    pub rw: f64,
    pub adjrw: f64,
    pub wtlos: f64,
    pub error_code: i32,
    pub warning_code: i32,
}

/// Worker health heartbeat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerHealth {
    pub worker_id: String,
    pub status: String,
    pub tasks_completed: u64,
    pub avg_processing_ms: f64,
    pub uptime_secs: u64,
    pub version: String,
    pub timestamp: String,
}

/// MQTT connection manager
pub struct MqttManager {
    client: AsyncClient,
    client_id: String,
    task_rx: mpsc::Receiver<DrgTask>,
}

pub struct MqttHandle {
    client: AsyncClient,
    client_id: String,
}

impl MqttHandle {
    /// Publish a DRG result to the results topic.
    pub async fn publish_result(&self, result: &DrgResult) -> Result<(), String> {
        let topic = format!("{}{}", TOPIC_RESULTS_PREFIX, result.request_id);
        let payload = serde_json::to_vec(result).map_err(|e| e.to_string())?;
        self.client
            .publish(&topic, QoS::AtLeastOnce, false, payload)
            .await
            .map_err(|e| format!("Failed to publish result: {}", e))
    }

    /// Publish worker health heartbeat.
    pub async fn publish_health(&self, health: &WorkerHealth) -> Result<(), String> {
        let topic = format!("{}{}", TOPIC_HEALTH_PREFIX, self.client_id);
        let payload = serde_json::to_vec(health).map_err(|e| e.to_string())?;
        self.client
            .publish(&topic, QoS::AtMostOnce, true, payload)
            .await
            .map_err(|e| format!("Failed to publish health: {}", e))
    }

    /// Publish worker registration (retained message).
    pub async fn register(&self, version: &str) -> Result<(), String> {
        let topic = format!("{}{}", TOPIC_REGISTER_PREFIX, self.client_id);
        let payload = serde_json::to_string(&serde_json::json!({
            "worker_id": self.client_id,
            "version": version,
            "registered_at": chrono::Utc::now().to_rfc3339(),
        }))
        .map_err(|e| e.to_string())?;
        self.client
            .publish(&topic, QoS::AtLeastOnce, true, payload.as_bytes())
            .await
            .map_err(|e| format!("Failed to register: {}", e))
    }

    /// Disconnect gracefully.
    pub async fn disconnect(&self) -> Result<(), String> {
        self.client
            .disconnect()
            .await
            .map_err(|e| format!("Failed to disconnect: {}", e))
    }
}

/// Connect to the MQTT broker using WorkerConfig and return a handle + task receiver.
pub async fn connect(
    config: &WorkerConfig,
) -> Result<(MqttHandle, mpsc::Receiver<DrgTask>), String> {
    let mut mqtt_options =
        MqttOptions::new(&config.client_id, &config.broker_host, config.broker_port);
    mqtt_options.set_keep_alive(Duration::from_secs(config.keep_alive_secs as u64));
    mqtt_options.set_clean_session(true);

    // Authentication
    if let (Some(user), Some(pass)) = (&config.mqtt_username, &config.mqtt_password) {
        if !user.is_empty() {
            mqtt_options.set_credentials(user, pass);
        }
    }

    // TLS
    if config.use_tls {
        let transport = match &config.tls_ca_cert_path {
            Some(ca_path) if !ca_path.is_empty() => {
                let ca = std::fs::read(ca_path)
                    .map_err(|e| format!("Failed to read CA certificate '{}': {}", ca_path, e))?;
                Transport::tls(ca, None, None)
            }
            _ => {
                // Use system default CA certificates
                Transport::tls_with_default_config()
            }
        };
        mqtt_options.set_transport(transport);
    }

    let (client, mut eventloop) = AsyncClient::new(mqtt_options, 100);

    // Subscribe to pending tasks (shared subscription)
    client
        .subscribe(TOPIC_PENDING, QoS::AtLeastOnce)
        .await
        .map_err(|e| format!("Failed to subscribe: {}", e))?;

    let (task_tx, task_rx) = mpsc::channel::<DrgTask>(100);

    // Spawn event loop handler
    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::Publish(publish))) => {
                    if publish.topic.contains("tasks/drg/pending") {
                        match serde_json::from_slice::<DrgTask>(&publish.payload) {
                            Ok(task) => {
                                log::info!("Received task: {}", task.request_id);
                                if task_tx.send(task).await.is_err() {
                                    log::error!("Task channel closed");
                                    break;
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to parse task: {}", e);
                            }
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    log::error!("MQTT connection error: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });

    Ok((
        MqttHandle {
            client,
            client_id: config.client_id.clone(),
        },
        task_rx,
    ))
}
