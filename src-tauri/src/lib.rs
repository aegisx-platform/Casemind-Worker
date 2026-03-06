mod config;
mod dbf;
mod exe_runner;
mod mqtt;
mod worker;

use config::WorkerConfig;
use std::sync::Arc;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State,
};
use tokio::sync::{mpsc, Mutex};
use worker::{TaskLogEntry, Worker, WorkerStatus};

/// Shared app state
struct AppState {
    worker: Arc<Mutex<Option<Worker>>>,
    config: Arc<Mutex<WorkerConfig>>,
    status_rx: Arc<Mutex<mpsc::Receiver<WorkerStatus>>>,
    status_tx: mpsc::Sender<WorkerStatus>,
}

// ── Tauri Commands ──────────────────────────────────────────────────

#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<WorkerConfig, String> {
    Ok(state.config.lock().await.clone())
}

#[tauri::command]
async fn save_config(state: State<'_, AppState>, config: WorkerConfig) -> Result<(), String> {
    config.save()?;
    *state.config.lock().await = config.clone();
    if let Some(w) = state.worker.lock().await.as_ref() {
        w.update_config(config).await?;
    }
    Ok(())
}

#[tauri::command]
async fn connect_worker(state: State<'_, AppState>) -> Result<(), String> {
    let config = state.config.lock().await.clone();
    let worker = Worker::new(config, state.status_tx.clone());
    worker.start().await?;
    *state.worker.lock().await = Some(worker);
    Ok(())
}

#[tauri::command]
async fn disconnect_worker(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(w) = state.worker.lock().await.as_ref() {
        w.stop().await?;
    }
    *state.worker.lock().await = None;
    Ok(())
}

#[tauri::command]
async fn pause_worker(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(w) = state.worker.lock().await.as_ref() {
        w.pause();
    }
    Ok(())
}

#[tauri::command]
async fn resume_worker(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(w) = state.worker.lock().await.as_ref() {
        w.resume();
    }
    Ok(())
}

#[tauri::command]
async fn get_status(state: State<'_, AppState>) -> Result<WorkerStatus, String> {
    if let Some(w) = state.worker.lock().await.as_ref() {
        Ok(w.get_status().await)
    } else {
        let config = state.config.lock().await;
        Ok(WorkerStatus {
            connected: false,
            paused: false,
            tasks_completed: 0,
            tasks_failed: 0,
            avg_processing_ms: 0.0,
            current_queue: 0,
            exe_available: config.has_valid_exe(),
            broker_url: config.broker_display_url(),
            worker_id: config.client_id.clone(),
            uptime_secs: 0,
        })
    }
}

#[tauri::command]
async fn get_task_log(state: State<'_, AppState>) -> Result<Vec<TaskLogEntry>, String> {
    if let Some(w) = state.worker.lock().await.as_ref() {
        Ok(w.get_task_log().await)
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
async fn send_test_task(
    state: State<'_, AppState>,
    cases: Vec<mqtt::DrgTaskCase>,
) -> Result<String, String> {
    let config = state.config.lock().await;
    let request_id = format!("test-{}", uuid::Uuid::new_v4());
    let version_id = config.version.clone();
    drop(config);

    let task = mqtt::DrgTask {
        request_id: request_id.clone(),
        version_id,
        cases,
        published_at: chrono::Utc::now().to_rfc3339(),
    };

    let worker_guard = state.worker.lock().await;
    let worker = worker_guard
        .as_ref()
        .ok_or("Worker not connected. Please connect first.")?;
    worker.publish_test_task(&task).await?;

    Ok(request_id)
}

#[tauri::command]
async fn check_exe_path(path: String) -> Result<bool, String> {
    let exe_path = std::path::PathBuf::from(&path).join("TGrp6305.exe");
    Ok(exe_path.exists())
}

#[tauri::command]
async fn download_exe(
    _app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    use tokio::io::AsyncWriteExt;

    let config = state.config.lock().await;
    let download_url = config.download_url.clone();
    drop(config);

    // Target directory: config_dir/exe/ (platform-specific)
    let target_dir = WorkerConfig::config_dir().join("exe");
    std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    // Download the file — stream to disk to handle large files (~40MB)
    let response = reqwest::get(&download_url)
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()));
    }

    let temp_path = target_dir.join("download.zip");

    // Stream response body to file
    {
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| format!("Failed to create temp file: {}", e))?;
        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Download stream error: {}", e))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("Write error: {}", e))?;
        }
        file.flush().await.map_err(|e| format!("Flush error: {}", e))?;
    }

    // Verify file size
    let metadata = std::fs::metadata(&temp_path).map_err(|e| e.to_string())?;
    log::info!("Downloaded {} bytes to {:?}", metadata.len(), temp_path);

    if metadata.len() < 1000 {
        let content = std::fs::read_to_string(&temp_path).unwrap_or_default();
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("Downloaded file too small ({}B), might be HTML: {}", metadata.len(), &content[..200.min(content.len())]));
    }

    // Extract ZIP
    let file = std::fs::File::open(&temp_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("invalid Zip archive: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let outpath = target_dir.join(file.name());

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = outpath.parent() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut outfile = std::fs::File::create(&outpath).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
        }
    }

    // Cleanup temp zip
    let _ = std::fs::remove_file(&temp_path);

    // Find TGrp6305.exe — it might be in a subfolder (e.g. TGrp6305/)
    let exe_name = "TGrp6305.exe";
    if target_dir.join(exe_name).exists() {
        return Ok(target_dir.to_string_lossy().to_string());
    }
    // Scan one level of subdirectories
    if let Ok(entries) = std::fs::read_dir(&target_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join(exe_name).exists() {
                return Ok(path.to_string_lossy().to_string());
            }
        }
    }

    Ok(target_dir.to_string_lossy().to_string())
}

// ── Tauri App Setup ─────────────────────────────────────────────────

pub fn run() {
    let config = WorkerConfig::load();
    let (status_tx, status_rx) = mpsc::channel::<WorkerStatus>(100);

    let app_state = AppState {
        worker: Arc::new(Mutex::new(None)),
        config: Arc::new(Mutex::new(config)),
        status_rx: Arc::new(Mutex::new(status_rx)),
        status_tx,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(|app| {
            // Auto-connect if configured
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = handle.state::<AppState>();
                    let auto_start = state.config.lock().await.auto_start;
                    if auto_start {
                        log::info!("Auto-starting worker...");
                        let config = state.config.lock().await.clone();
                        let worker = Worker::new(config, state.status_tx.clone());
                        match worker.start().await {
                            Ok(()) => {
                                *state.worker.lock().await = Some(worker);
                                log::info!("Worker auto-started successfully");
                            }
                            Err(e) => {
                                log::error!("Auto-start failed: {}", e);
                            }
                        }
                    }
                });
            }

            // Build tray menu
            let show = MenuItemBuilder::with_id("show", "Show Dashboard").build(app)?;
            let pause = MenuItemBuilder::with_id("pause", "Pause").build(app)?;
            let resume = MenuItemBuilder::with_id("resume", "Resume").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .items(&[&show, &pause, &resume, &quit])
                .build()?;

            let _tray = TrayIconBuilder::new()
                .tooltip("CaseMind Worker — Idle")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "pause" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app.state::<AppState>();
                            let guard = state.worker.lock().await;
                            if let Some(w) = guard.as_ref() {
                                w.pause();
                            }
                        });
                    }
                    "resume" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app.state::<AppState>();
                            let guard = state.worker.lock().await;
                            if let Some(w) = guard.as_ref() {
                                w.resume();
                            }
                        });
                    }
                    "quit" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app.state::<AppState>();
                            {
                                let guard = state.worker.lock().await;
                                if let Some(w) = guard.as_ref() {
                                    let _ = w.stop().await;
                                }
                            }
                            app.exit(0);
                        });
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            connect_worker,
            disconnect_worker,
            pause_worker,
            resume_worker,
            get_status,
            get_task_log,
            send_test_task,
            check_exe_path,
            download_exe,
        ])
        .run(tauri::generate_context!())
        .expect("error while running casemind-worker");
}
