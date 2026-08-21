use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct RuntimeStatus {
    pub wsl_available: bool,
    pub nvidia_available: bool,
    pub cuda_visible: bool,
    pub free_disk_gb: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct RuntimeManager {
    pub workspace: PathBuf,
}

impl RuntimeManager {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }

    pub fn inspect(&self) -> RuntimeStatus {
        RuntimeStatus {
            wsl_available: command_exists("wsl"),
            nvidia_available: command_exists("nvidia-smi"),
            cuda_visible: false,
            free_disk_gb: None,
        }
    }

    pub fn required_directories(&self) -> Vec<PathBuf> {
        vec![
            self.workspace.join("models"),
            self.workspace.join("projects"),
            self.workspace.join("runtime"),
            self.workspace.join("outputs"),
        ]
    }

    pub fn create_layout(&self) -> std::io::Result<()> {
        for dir in self.required_directories() {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|_| true)
        .unwrap_or(false)
}
