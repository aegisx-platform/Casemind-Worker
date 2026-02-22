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

    /// Create an isolated temp directory with symlinks/copies of all exe files.
    /// This prevents VFP file lock conflicts when running multiple exe instances.
    fn create_isolated_workdir(&self) -> Result<PathBuf, String> {
        let unique_id = uuid::Uuid::new_v4();
        let work_dir = std::env::temp_dir().join(format!("casemind-worker-{}", unique_id));
        std::fs::create_dir_all(&work_dir)
            .map_err(|e| format!("Failed to create work dir: {}", e))?;

        // Link/copy all files from exe_dir into work_dir
        let entries = std::fs::read_dir(&self.exe_dir)
            .map_err(|e| format!("Failed to read exe dir: {}", e))?;

        for entry in entries.flatten() {
            let src = entry.path();
            if !src.is_file() {
                continue;
            }
            let filename = entry.file_name();
            let dest = work_dir.join(&filename);

            // Try symlink first (fast, no disk usage), fall back to copy
            #[cfg(unix)]
            {
                if std::os::unix::fs::symlink(&src, &dest).is_err() {
                    std::fs::copy(&src, &dest)
                        .map_err(|e| format!("Failed to copy {:?}: {}", filename, e))?;
                }
            }
            #[cfg(windows)]
            {
                // On Windows, use hard links (no admin needed) or copy
                if std::fs::hard_link(&src, &dest).is_err() {
                    std::fs::copy(&src, &dest)
                        .map_err(|e| format!("Failed to copy {:?}: {}", filename, e))?;
                }
            }
        }

        Ok(work_dir)
    }

    /// Clean up an isolated work directory
    fn cleanup_workdir(work_dir: &Path) {
        let _ = std::fs::remove_dir_all(work_dir);
    }

    /// Run TGrp6305.exe on a batch of records.
    /// Creates an isolated temp directory, runs exe, reads results.
    pub async fn run(&self, records: &[ExeDbfRecord]) -> Result<Vec<ExeDbfRecord>, String> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| format!("Semaphore error: {}", e))?;

        // Create isolated working directory to prevent VFP file lock conflicts
        let work_dir = self.create_isolated_workdir()?;
        let dbf_filename = "input.dbf".to_string();
        let dbf_path = work_dir.join(&dbf_filename);

        // Create DBF with input records
        dbf::create_dbf(&dbf_path, records)?;

        // Run exe in the isolated directory
        let result = self.execute_exe(&work_dir, &dbf_filename).await;

        // Read results
        let output = match &result {
            Ok(_) => dbf::read_dbf(&dbf_path),
            Err(e) => {
                Self::cleanup_workdir(&work_dir);
                return Err(e.clone());
            }
        };

        // Clean up isolated directory
        Self::cleanup_workdir(&work_dir);

        output
    }

    async fn execute_exe(&self, work_dir: &Path, dbf_filename: &str) -> Result<(), String> {
        let exe_path = work_dir.join("TGrp6305.exe");
        if !exe_path.exists() {
            return Err(format!("TGrp6305.exe not found at {:?}", exe_path));
        }

        let work_dir = work_dir.to_path_buf();
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
                    .current_dir(&work_dir)
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
                    .current_dir(&work_dir)
                    .env("WINEDEBUG", "-all")
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
