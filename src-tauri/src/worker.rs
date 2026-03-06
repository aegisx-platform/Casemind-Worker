use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};

use crate::config::WorkerConfig;
use crate::dbf;
use crate::exe_runner::ExeRunner;
use crate::mqtt::{self, DrgResult, DrgResultCase, DrgTask, MqttHandle, WorkerHealth};

/// Worker status for UI display
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkerStatus {
    pub connected: bool,
    pub paused: bool,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub avg_processing_ms: f64,
    pub current_queue: usize,
    pub exe_available: bool,
    pub broker_url: String,
    pub worker_id: String,
    pub uptime_secs: u64,
}

/// A completed task entry for the log
#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskLogEntry {
    pub request_id: String,
    pub case_count: usize,
    pub drg_codes: Vec<String>,
    pub processing_ms: u64,
    pub status: String,
    pub completed_at: String,
    pub request_data: Option<serde_json::Value>,
    pub response_data: Option<serde_json::Value>,
}

/// Main worker orchestrator
pub struct Worker {
    config: Arc<Mutex<WorkerConfig>>,
    exe_runner: Arc<ExeRunner>,
    mqtt_handle: Arc<Mutex<Option<MqttHandle>>>,
    connected: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    tasks_completed: Arc<AtomicU64>,
    tasks_failed: Arc<AtomicU64>,
    total_processing_ms: Arc<AtomicU64>,
    start_time: Instant,
    task_log: Arc<Mutex<Vec<TaskLogEntry>>>,
    status_tx: mpsc::Sender<WorkerStatus>,
}

impl Worker {
    pub fn new(config: WorkerConfig, status_tx: mpsc::Sender<WorkerStatus>) -> Self {
        let exe_runner = ExeRunner::new(&config.exe_base_path, config.max_concurrent);
        Self {
            config: Arc::new(Mutex::new(config)),
            exe_runner: Arc::new(exe_runner),
            mqtt_handle: Arc::new(Mutex::new(None)),
            connected: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            tasks_completed: Arc::new(AtomicU64::new(0)),
            tasks_failed: Arc::new(AtomicU64::new(0)),
            total_processing_ms: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
            task_log: Arc::new(Mutex::new(Vec::new())),
            status_tx,
        }
    }

    /// Start the worker: connect to MQTT and begin processing tasks.
    pub async fn start(&self) -> Result<(), String> {
        let config = self.config.lock().await;
        let client_id = config.client_id.clone();
        let version = config.version.clone();
        let (handle, mut task_rx) = mqtt::connect(&config).await?;
        drop(config);

        // Register with broker
        handle.register(&version).await?;

        self.connected.store(true, Ordering::SeqCst);
        *self.mqtt_handle.lock().await = Some(handle);

        self.emit_status().await;

        // Start heartbeat loop
        let health_handle = self.mqtt_handle.clone();
        let health_connected = self.connected.clone();
        let health_completed = self.tasks_completed.clone();
        let health_total_ms = self.total_processing_ms.clone();
        let health_start = self.start_time;
        let health_client_id = client_id.clone();
        let health_version = version.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                if !health_connected.load(Ordering::SeqCst) {
                    break;
                }
                let completed = health_completed.load(Ordering::SeqCst);
                let total_ms = health_total_ms.load(Ordering::SeqCst);
                let avg_ms = if completed > 0 {
                    total_ms as f64 / completed as f64
                } else {
                    0.0
                };
                let health = WorkerHealth {
                    worker_id: health_client_id.clone(),
                    status: "active".to_string(),
                    tasks_completed: completed,
                    avg_processing_ms: avg_ms,
                    uptime_secs: health_start.elapsed().as_secs(),
                    version: health_version.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                if let Some(h) = health_handle.lock().await.as_ref() {
                    let _ = h.publish_health(&health).await;
                }
            }
        });

        // Process tasks
        let exe_runner = self.exe_runner.clone();
        let mqtt_handle = self.mqtt_handle.clone();
        let connected = self.connected.clone();
        let paused = self.paused.clone();
        let tasks_completed = self.tasks_completed.clone();
        let tasks_failed = self.tasks_failed.clone();
        let total_processing_ms = self.total_processing_ms.clone();
        let task_log = self.task_log.clone();
        let status_tx = self.status_tx.clone();
        let worker_id = client_id.clone();

        tokio::spawn(async move {
            while let Some(task) = task_rx.recv().await {
                if !connected.load(Ordering::SeqCst) {
                    break;
                }
                if paused.load(Ordering::SeqCst) {
                    continue;
                }

                // Spawn each task concurrently — ExeRunner's semaphore controls max parallelism
                let exe_runner = exe_runner.clone();
                let mqtt_handle = mqtt_handle.clone();
                let tasks_completed = tasks_completed.clone();
                let tasks_failed = tasks_failed.clone();
                let total_processing_ms = total_processing_ms.clone();
                let task_log = task_log.clone();
                let status_tx = status_tx.clone();
                let worker_id = worker_id.clone();
                let connected = connected.clone();
                let paused = paused.clone();

                tokio::spawn(async move {
                    let start = Instant::now();
                    let result = process_task(&exe_runner, &task, &worker_id).await;
                    let elapsed_ms = start.elapsed().as_millis() as u64;

                    match result {
                        Ok(drg_result) => {
                            // Publish result
                            if let Some(h) = mqtt_handle.lock().await.as_ref() {
                                if let Err(e) = h.publish_result(&drg_result).await {
                                    log::error!("Failed to publish result: {}", e);
                                }
                            }

                            tasks_completed.fetch_add(1, Ordering::SeqCst);
                            total_processing_ms.fetch_add(elapsed_ms, Ordering::SeqCst);

                            // Add to log with request/response data
                            let entry = TaskLogEntry {
                                request_id: task.request_id.clone(),
                                case_count: task.cases.len(),
                                drg_codes: drg_result
                                    .cases
                                    .iter()
                                    .map(|c| c.drg.clone())
                                    .collect(),
                                processing_ms: elapsed_ms,
                                status: "success".to_string(),
                                completed_at: chrono::Utc::now().to_rfc3339(),
                                request_data: serde_json::to_value(&task).ok(),
                                response_data: serde_json::to_value(&drg_result).ok(),
                            };
                            let mut log = task_log.lock().await;
                            log.insert(0, entry);
                            if log.len() > 50 {
                                log.truncate(50);
                            }
                        }
                        Err(e) => {
                            log::error!("Task {} failed: {}", task.request_id, e);
                            tasks_failed.fetch_add(1, Ordering::SeqCst);

                            let entry = TaskLogEntry {
                                request_id: task.request_id.clone(),
                                case_count: task.cases.len(),
                                drg_codes: vec![],
                                processing_ms: elapsed_ms,
                                status: format!("error: {}", e),
                                completed_at: chrono::Utc::now().to_rfc3339(),
                                request_data: serde_json::to_value(&task).ok(),
                                response_data: None,
                            };
                            let mut log = task_log.lock().await;
                            log.insert(0, entry);
                            if log.len() > 50 {
                                log.truncate(50);
                            }
                        }
                    }

                    // Emit updated status to UI
                    let completed = tasks_completed.load(Ordering::SeqCst);
                    let failed = tasks_failed.load(Ordering::SeqCst);
                    let total_ms = total_processing_ms.load(Ordering::SeqCst);
                    let _ = status_tx
                        .send(WorkerStatus {
                            connected: connected.load(Ordering::SeqCst),
                            paused: paused.load(Ordering::SeqCst),
                            tasks_completed: completed,
                            tasks_failed: failed,
                            avg_processing_ms: if completed > 0 {
                                total_ms as f64 / completed as f64
                            } else {
                                0.0
                            },
                            current_queue: 0,
                            exe_available: exe_runner.is_available(),
                            broker_url: String::new(),
                            worker_id: worker_id.clone(),
                            uptime_secs: 0,
                        })
                        .await;
                });
            }
        });

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        self.connected.store(false, Ordering::SeqCst);
        if let Some(h) = self.mqtt_handle.lock().await.take() {
            h.disconnect().await?;
        }
        self.emit_status().await;
        Ok(())
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub async fn get_status(&self) -> WorkerStatus {
        let config = self.config.lock().await;
        let completed = self.tasks_completed.load(Ordering::SeqCst);
        let total_ms = self.total_processing_ms.load(Ordering::SeqCst);
        WorkerStatus {
            connected: self.connected.load(Ordering::SeqCst),
            paused: self.paused.load(Ordering::SeqCst),
            tasks_completed: completed,
            tasks_failed: self.tasks_failed.load(Ordering::SeqCst),
            avg_processing_ms: if completed > 0 {
                total_ms as f64 / completed as f64
            } else {
                0.0
            },
            current_queue: 0,
            exe_available: self.exe_runner.is_available(),
            broker_url: config.broker_display_url(),
            worker_id: config.client_id.clone(),
            uptime_secs: self.start_time.elapsed().as_secs(),
        }
    }

    pub async fn get_task_log(&self) -> Vec<TaskLogEntry> {
        self.task_log.lock().await.clone()
    }

    pub async fn update_config(&self, new_config: WorkerConfig) -> Result<(), String> {
        new_config.save()?;
        *self.config.lock().await = new_config;
        Ok(())
    }

    /// Publish a test task to the pending topic via the existing MQTT connection.
    pub async fn publish_test_task(&self, task: &DrgTask) -> Result<(), String> {
        let guard = self.mqtt_handle.lock().await;
        let handle = guard.as_ref().ok_or("MQTT not connected")?;
        handle.publish_task(task).await
    }

    async fn emit_status(&self) {
        let status = self.get_status().await;
        let _ = self.status_tx.send(status).await;
    }
}

/// Process a single DRG task: convert to DBF records, run exe, collect results.
async fn process_task(
    exe_runner: &ExeRunner,
    task: &DrgTask,
    worker_id: &str,
) -> Result<DrgResult, String> {
    let start = Instant::now();

    // Convert task cases to DBF records
    let records: Vec<_> = task
        .cases
        .iter()
        .map(|c| {
            dbf::input_to_record(
                &c.pdx,
                &c.sdx,
                &c.procedures,
                c.age,
                c.age_in_days,
                &c.sex,
                c.discharge_type,
                c.los,
                c.admission_weight,
            )
        })
        .collect();

    // Run exe
    let results = exe_runner.run(&records).await?;

    // Convert results
    let cases: Vec<DrgResultCase> = results
        .iter()
        .map(|r| DrgResultCase {
            drg: r.drg.clone(),
            mdc: r.mdc.clone(),
            rw: r.rw,
            adjrw: r.adjrw,
            wtlos: r.wtlos,
            error_code: r.err,
            warning_code: r.warn,
        })
        .collect();

    Ok(DrgResult {
        request_id: task.request_id.clone(),
        worker_id: worker_id.to_string(),
        version_id: task.version_id.clone(),
        cases,
        processing_time_ms: start.elapsed().as_millis() as u64,
        completed_at: chrono::Utc::now().to_rfc3339(),
    })
}
