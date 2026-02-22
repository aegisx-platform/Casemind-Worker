use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::Semaphore;

use crate::dbf::{self, ExeDbfRecord};

/// Manages TGrp6305.exe subprocess execution with concurrency control.
pub struct ExeRunner {
    exe_dir: PathBuf,
    semaphore: Semaphore,
    timeout: Duration,
}

impl ExeRunner {
    pub fn new(exe_dir: &str, max_concurrent: usize) -> Self {
        Self {
            exe_dir: PathBuf::from(exe_dir),
            semaphore: Semaphore::new(max_concurrent),
            timeout: Duration::from_secs(30),
        }
    }

    pub fn exe_path(&self) -> PathBuf {
        self.exe_dir.join("TGrp6305.exe")
    }

    pub fn is_available(&self) -> bool {
        self.exe_path().exists()
    }

    /// Run TGrp6305.exe on a batch of records.
    /// Creates a temporary DBF, runs exe, reads results.
    pub async fn run(&self, records: &[ExeDbfRecord]) -> Result<Vec<ExeDbfRecord>, String> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| format!("Semaphore error: {}", e))?;

        let unique_id = uuid::Uuid::new_v4();
        let dbf_filename = format!("worker_{}.dbf", unique_id);
        let dbf_path = self.exe_dir.join(&dbf_filename);

        // Create DBF with input records
        dbf::create_dbf(&dbf_path, records)?;

        // Run exe
        let result = self.execute_exe(&dbf_path, &dbf_filename).await;

        // Read results (even if exe had issues, try to read)
        let output = match &result {
            Ok(_) => dbf::read_dbf(&dbf_path),
            Err(e) => {
                // Clean up and propagate error
                let _ = std::fs::remove_file(&dbf_path);
                return Err(e.clone());
            }
        };

        // Clean up temp DBF
        let _ = std::fs::remove_file(&dbf_path);

        output
    }

    async fn execute_exe(&self, _dbf_path: &Path, dbf_filename: &str) -> Result<(), String> {
        let exe_path = self.exe_path();
        if !exe_path.exists() {
            return Err(format!("TGrp6305.exe not found at {:?}", exe_path));
        }

        let exe_dir = self.exe_dir.clone();
        let dbf_name = dbf_filename.to_string();

        // Run in a blocking task since it's a subprocess
        let result = tokio::time::timeout(self.timeout, tokio::task::spawn_blocking(move || {
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;

                let output = std::process::Command::new(&exe_path)
                    .arg(&dbf_name)
                    .arg("0")
                    .current_dir(&exe_dir)
                    .creation_flags(CREATE_NO_WINDOW)
                    .output()
                    .map_err(|e| format!("Failed to execute TGrp6305.exe: {}", e))?;

                if !output.status.success() {
                    return Err(format!(
                        "TGrp6305.exe exited with code: {:?}, stderr: {}",
                        output.status.code(),
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }
                Ok(())
            }

            #[cfg(not(target_os = "windows"))]
            {
                // On non-Windows, attempt via Wine (for development/testing only)
                let output = std::process::Command::new("wine")
                    .arg(exe_path.to_str().unwrap())
                    .arg(&dbf_name)
                    .arg("0")
                    .current_dir(&exe_dir)
                    .output()
                    .map_err(|e| format!("Failed to execute via Wine: {}", e))?;

                if !output.status.success() {
                    return Err(format!(
                        "Wine/TGrp6305.exe exited with code: {:?}",
                        output.status.code()
                    ));
                }
                Ok(())
            }
        }))
        .await;

        match result {
            Ok(Ok(inner)) => inner,
            Ok(Err(e)) => Err(format!("Task join error: {}", e)),
            Err(_) => Err("TGrp6305.exe timed out after 30 seconds".to_string()),
        }
    }
}
