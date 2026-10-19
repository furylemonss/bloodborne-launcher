use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::constants::STEPS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Idle,
    Running,
    Paused,
    Extracting,
    Ready,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Skipped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepState {
    pub id: String,
    pub title: String,
    pub status: StepStatus,
    pub detail: Option<String>,
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerState {
    pub root: Option<String>,
    pub status: Status,
    pub current_step: usize,
    pub steps: Vec<StepState>,
    pub last_error: Option<String>,
    pub completed: bool,
    pub user_paused: bool,
}

impl Default for InstallerState {
    fn default() -> Self {
        Self {
            root: None,
            status: Status::Idle,
            current_step: 0,
            steps: STEPS
                .iter()
                .map(|(id, title)| StepState {
                    id: id.to_string(),
                    title: title.to_string(),
                    status: StepStatus::Pending,
                    detail: None,
                    percent: 0,
                })
                .collect(),
            last_error: None,
            completed: false,
            user_paused: false,
        }
    }
}

impl InstallerState {
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, path)
    }
}

pub struct Control {
    pub paused: AtomicBool,
    pub cancel: AtomicBool,
    pub running: AtomicBool,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            paused: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            running: AtomicBool::new(false),
        }
    }
}

impl Control {
    pub fn reset_for_run(&self) {
        self.paused.store(false, Ordering::SeqCst);
        self.cancel.store(false, Ordering::SeqCst);
    }

    pub fn request_pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    pub fn request_resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn should_stop(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
}

pub fn state_file(root: &Path) -> PathBuf {
    root.join("installer-state.json")
}

pub fn downloads_dir(root: &Path) -> PathBuf {
    root.join("downloads")
}

pub fn tools_dir(root: &Path) -> PathBuf {
    root.join("tools")
}

pub fn game_dir(root: &Path) -> PathBuf {
    // Tam oyun ağacı burada: downloads\CUSA03173
    root.join("downloads").join("CUSA03173")
}

pub fn patch_dir(root: &Path) -> PathBuf {
    // Tam update ağacı burada: downloads\CUSA03173-patch
    root.join("downloads").join("CUSA03173-patch")
}

pub fn port_dir(root: &Path) -> PathBuf {
    root.join("port")
}

pub fn ensure_layout(root: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(downloads_dir(root))?;
    std::fs::create_dir_all(tools_dir(root))?;
    std::fs::create_dir_all(game_dir(root))?;
    std::fs::create_dir_all(patch_dir(root))?;
    std::fs::create_dir_all(port_dir(root))?;
    adopt_existing(root);
    Ok(())
}

pub fn adopt_existing(root: &Path) {
    let downloads = downloads_dir(root);
    let names = [
        crate::constants::BASE_PKG_NAME,
        crate::constants::UPDATE_PKG_NAME,
        crate::constants::PORT_ZIP_NAME,
        crate::constants::EXTRACTOR_ZIP_NAME,
    ];
    for name in names {
        let dest = downloads.join(name);
        if dest.exists() {
            continue;
        }
        let src = root.join(name);
        if src.is_file() {
            let _ = std::fs::rename(&src, &dest);
            continue;
        }
        let parent = root.parent().map(Path::to_path_buf);
        if let Some(parent) = parent {
            let src = parent.join(name);
            if src.is_file() {
                let _ = std::fs::rename(&src, &dest);
            }
        }
    }
}
