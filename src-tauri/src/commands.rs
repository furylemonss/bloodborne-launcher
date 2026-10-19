use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager};

use crate::download::free_space;
use crate::extract::{
    game_exe, play_exe, read_port_settings, write_port_settings, PortSettings,
};
use crate::notify;
use crate::pipeline::{detect_ready, run_pipeline, summarize_steps, PipelineCtx};
use crate::state::{
    adopt_existing, downloads_dir, ensure_layout, game_dir, port_dir, state_file, tools_dir, Control,
    InstallerState, Status, StepStatus,
};
use crate::{power, tray};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[derive(Clone, serde::Serialize)]
pub struct DiskInfo {
    pub path: String,
    pub free_bytes: u64,
    pub total_required: u64,
    pub enough: bool,
}

pub struct Shared {
    pub state: Arc<Mutex<InstallerState>>,
    pub control: Arc<Control>,
    pub root: Arc<Mutex<Option<PathBuf>>>,
    pub game_running: Arc<std::sync::atomic::AtomicBool>,
    pub game_pid: Arc<Mutex<Option<u32>>>,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(InstallerState::default())),
            control: Arc::new(Control::default()),
            root: Arc::new(Mutex::new(None)),
            game_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            game_pid: Arc::new(Mutex::new(None)),
        }
    }
}

fn persist(app: &AppHandle) {
    let shared = app.state::<Shared>();
    let root = match shared.root.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => None,
    };
    if let Some(root) = root {
        if let Ok(state) = shared.state.lock() {
            let _ = state.save(&state_file(&root));
        }
    }
}

fn emit_state(app: &AppHandle) {
    let shared = app.state::<Shared>();
    let snapshot = match shared.state.lock() {
        Ok(state) => Some(state.clone()),
        Err(_) => None,
    };
    if let Some(state) = snapshot {
        let _ = app.emit("pipeline://progress", state);
    }
}

#[tauri::command]
pub fn get_state(app: AppHandle) -> InstallerState {
    let shared = app.state::<Shared>();
    shared.state.lock().map(|s| s.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn set_root(app: AppHandle, path: String) -> Result<InstallerState, String> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err("klasör bulunamadı".into());
    }
    ensure_layout(&root).map_err(|e| e.to_string())?;
    adopt_existing(&root);
    let mut state = {
        let path = state_file(&root);
        if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<InstallerState>(&text).ok())
                .filter(|s| s.steps.len() == crate::constants::STEPS.len())
                .unwrap_or_default()
        } else {
            InstallerState::default()
        }
    };
    state.root = Some(root.to_string_lossy().to_string());
    summarize_steps(&mut state, &root);
    if detect_ready(&root) {
        state.completed = true;
        state.status = Status::Ready;
    }

    let shared = app.state::<Shared>();
    *shared.root.lock().map_err(|e| e.to_string())? = Some(root.clone());
    *shared.state.lock().map_err(|e| e.to_string())? = state.clone();
    persist(&app);
    if let Some(last) = dirs::data_dir().map(|d| d.join("bloodborne-installer").join("last-root.txt")) {
        if let Some(parent) = last.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(last, root.to_string_lossy().as_bytes());
    }
    emit_state(&app);
    Ok(state)
}

#[tauri::command]
pub fn disk_info(_app: AppHandle, path: String) -> DiskInfo {
    let root = PathBuf::from(&path);
    let free = free_space(&root);
    let required = if root.is_dir() {
        crate::pipeline::required_free_bytes(&root)
    } else {
        crate::constants::REQUIRED_FREE_BYTES
    };
    DiskInfo {
        path,
        free_bytes: free,
        total_required: required,
        enough: free >= required,
    }
}

#[tauri::command]
pub fn start_pipeline(app: AppHandle) -> Result<(), String> {
    let shared = app.state::<Shared>();
    let root = shared
        .root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "kurulum yolu seçilmedi".to_string())?;

    if shared.control.running.load(Ordering::SeqCst) {
        return Err("zaten çalışıyor".into());
    }

    {
        let mut state = shared.state.lock().map_err(|e| e.to_string())?;
        if state.completed && detect_ready(&root) {
            return Err("kurulum zaten tamam".into());
        }
        state.status = Status::Running;
        state.last_error = None;
        state.user_paused = false;
        let snapshot = state.clone();
        drop(state);
        let _ = app.emit("pipeline://progress", snapshot);
    }
    persist(&app);

    let emit_app = app.clone();
    let emit_fn = Arc::new(move |event: &str, payload: serde_json::Value| {
        let _ = emit_app.emit(event, payload);
    }) as Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>;
    crate::pipeline::register_emit_sink(emit_fn);
    let ctx = Arc::new(PipelineCtx {
        root,
        state: shared.state.clone(),
        control: shared.control.clone(),
        app,
    });

    tauri::async_runtime::spawn(async move {
        let _ = run_pipeline(ctx).await;
    });
    Ok(())
}

pub fn pause_pipeline(app: AppHandle) {
    let shared = app.state::<Shared>();
    shared.control.request_pause();
    if let Ok(mut state) = shared.state.lock() {
        if state.status == Status::Running || state.status == Status::Extracting {
            state.status = Status::Paused;
            state.user_paused = true;
            let snapshot = state.clone();
            drop(state);
            let _ = app.emit("pipeline://progress", snapshot);
        }
    }
    persist(&app);
}

pub fn resume_pipeline(app: AppHandle) {
    let shared = app.state::<Shared>();
    if shared.control.running.load(Ordering::SeqCst) {
        shared.control.request_resume();
        if let Ok(mut state) = shared.state.lock() {
            if state.status == Status::Paused {
                state.status = if shared.control.is_paused() {
                    Status::Paused
                } else {
                    Status::Running
                };
                state.user_paused = false;
                let snapshot = state.clone();
                drop(state);
                let _ = app.emit("pipeline://progress", snapshot);
            }
        }
        persist(&app);
    } else {
        let _ = start_pipeline(app);
    }
}

#[tauri::command]
pub fn pause(app: AppHandle) {
    pause_pipeline(app);
}

#[tauri::command]
pub fn resume(app: AppHandle) {
    resume_pipeline(app);
}

#[tauri::command]
pub fn hide_to_tray(app: AppHandle) {
    tray::hide_main(&app);
}

#[tauri::command]
pub fn show_window(app: AppHandle) {
    tray::show_main(&app);
}

#[derive(serde::Serialize)]
pub struct CleanReport {
    pub removed: Vec<String>,
    pub freed: u64,
}

#[tauri::command]
pub fn clean_leftovers(
    app: AppHandle,
    delete_pkgs: bool,
    delete_extracted: bool,
    delete_extractor: bool,
    delete_patch: bool,
) -> Result<CleanReport, String> {
    let shared = app.state::<Shared>();
    let root = shared
        .root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("kurulum yolu yok")?;

    let mut removed = Vec::new();
    let mut freed = 0u64;

    let mut take = |path: PathBuf| -> u64 {
        let size = dir_size(&path);
        if path.exists() {
            let _ = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            removed.push(path.to_string_lossy().to_string());
        }
        size
    };

    if delete_pkgs {
        let downloads = downloads_dir(&root);
        for name in [
            crate::constants::BASE_PKG_NAME,
            crate::constants::UPDATE_PKG_NAME,
            crate::constants::PORT_ZIP_NAME,
            crate::constants::EXTRACTOR_ZIP_NAME,
        ] {
            freed += take(downloads.join(name));
        }
        if let Ok(entries) = std::fs::read_dir(&downloads) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().map(|e| e == "part").unwrap_or(false) {
                    freed += take(p);
                }
            }
        }
    } else {
        let downloads = downloads_dir(&root);
        if let Ok(entries) = std::fs::read_dir(&downloads) {
            for entry in entries.flatten() {
                let p = entry.path();
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if p.extension().map(|e| e == "part").unwrap_or(false)
                    || name == crate::constants::PORT_ZIP_NAME
                    || name == crate::constants::EXTRACTOR_ZIP_NAME
                {
                    freed += take(p);
                }
            }
        }
    }

    if delete_patch {
        freed += take(crate::state::patch_dir(&root));
    }

    if delete_extracted {
        freed += take(game_dir(&root));
        freed += take(port_dir(&root));
        if let Ok(mut state) = shared.state.lock() {
            state.completed = false;
            state.status = Status::Idle;
            for step in state.steps.iter_mut() {
                step.status = StepStatus::Pending;
                step.detail = None;
                step.percent = 0;
            }
            let snapshot = state.clone();
            drop(state);
            let _ = app.emit("pipeline://progress", snapshot);
        }
    }

    if delete_extractor {
        freed += take(tools_dir(&root));
    }

    persist(&app);
    emit_state(&app);
    Ok(CleanReport { removed, freed })
}

fn dir_size(path: &PathBuf) -> u64 {
    if path.is_file() {
        return std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            total += dir_size(&entry.path());
        }
    }
    total
}

fn no_window() -> u32 {
    0x0800_0000 // CREATE_NO_WINDOW
}

fn find_pids_by_image(name: &str) -> Vec<u32> {
    let mut pids = Vec::new();
    let Ok(out) = Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {name}"), "/FO", "CSV", "/NH"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(no_window())
        .output()
    else {
        return pids;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        // "name","PID","Session","Session#","Mem Usage"
        let cols: Vec<&str> = line.split(',').map(|c| c.trim_matches('"')).collect();
        if cols.len() >= 2 {
            if let Ok(pid) = cols[1].parse::<u32>() {
                pids.push(pid);
            }
        }
    }
    pids
}

fn pid_alive(pid: u32) -> bool {
    let Ok(out) = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(no_window())
        .output()
    else {
        return false;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    text.contains(&format!("{pid}\""))
        || text.lines().any(|l| {
            l.split(',')
                .nth(1)
                .map(|c| c.trim_matches('"') == pid.to_string())
                .unwrap_or(false)
        })
}

fn taskkill_pid(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(no_window())
        .status();
}

fn spawn_tracked(exe: &Path) -> Result<std::process::Child, String> {
    let dir = exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    std::process::Command::new(exe)
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(no_window())
        .spawn()
        .map_err(|e| format!("{} başlatılamadı: {e}", exe.file_name().unwrap_or_default().to_string_lossy()))
}

/// Oyna → Play Bloodborne.exe. Launcher erken çıkarsa Bloodborne.exe bitene kadar bekle.
#[tauri::command]
pub async fn play(app: AppHandle) -> Result<(), String> {
    let shared = app.state::<Shared>();
    let root = shared
        .root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("kurulum yolu yok")?;

    let exe = play_exe(&root);
    if !exe.exists() {
        return Err("Play Bloodborne.exe bulunamadı".into());
    }
    if shared.game_running.swap(true, Ordering::SeqCst) {
        return Err("oyun zaten açık".into());
    }

    let mut child = match spawn_tracked(&exe) {
        Ok(c) => c,
        Err(e) => {
            shared.game_running.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };
    let play_pid = child.id();
    if let Ok(mut g) = shared.game_pid.lock() {
        *g = Some(play_pid);
    }

    let handle = app.clone();
    let running = shared.game_running.clone();
    let pid_slot = shared.game_pid.clone();
    std::thread::spawn(move || {
        let _ = handle.emit("game://started", serde_json::json!({ "exe": "play" }));
        let started = std::time::Instant::now();
        let _ = child.wait();
        // Play kısa sürede kapandıysa oyun process'i hâlâ ayakta olabilir
        if started.elapsed() < std::time::Duration::from_secs(8) {
            let mut extra: Vec<u32> = find_pids_by_image("Bloodborne.exe")
                .into_iter()
                .filter(|p| *p != play_pid)
                .collect();
            if !extra.is_empty() {
                if let Ok(mut g) = pid_slot.lock() {
                    *g = extra.first().copied();
                }
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6 * 60 * 60);
                while std::time::Instant::now() < deadline {
                    extra.retain(|p| pid_alive(*p));
                    if extra.is_empty() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
        if let Ok(mut g) = pid_slot.lock() {
            *g = None;
        }
        running.store(false, Ordering::SeqCst);
        notify::game_exited(&handle);
        let _ = handle.emit("game://exited", serde_json::json!({}));
    });
    Ok(())
}

/// Durdur: Play ve Bloodborne.exe ağaçlarını kapat.
#[tauri::command]
pub fn stop_game(app: AppHandle) -> Result<(), String> {
    let shared = app.state::<Shared>();
    if let Ok(mut g) = shared.game_pid.lock() {
        if let Some(pid) = g.take() {
            taskkill_pid(pid);
        }
    }
    for pid in find_pids_by_image("Bloodborne.exe") {
        taskkill_pid(pid);
    }
    for pid in find_pids_by_image("Play Bloodborne.exe") {
        taskkill_pid(pid);
    }
    // wait thread temizleyene kadar kısa bekle
    for _ in 0..20 {
        if !shared.game_running.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Ok(())
}

/// Gelişmiş: Bloodborne.exe (oyun içi ayarlar/hileler/başarımlar/online paneli gibi).
#[tauri::command]
pub fn open_advanced(app: AppHandle) -> Result<(), String> {
    let shared = app.state::<Shared>();
    let root = shared
        .root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("kurulum yolu yok")?;
    let exe = game_exe(&root);
    if !exe.exists() {
        return Err("Bloodborne.exe bulunamadı".into());
    }
    spawn_tracked(&exe)?;
    Ok(())
}

/// Yüklendikten sonra yalnızca geçici artıklar: .part, port/extractor zip, tools.
#[tauri::command]
pub fn clean_temp(app: AppHandle) -> Result<CleanReport, String> {
    let shared = app.state::<Shared>();
    let root = shared
        .root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("kurulum yolu yok")?;

    let completed = {
        let st = shared.state.lock().map(|s| s.completed).unwrap_or(false);
        st || detect_ready(&root)
    };
    if !completed {
        return Err("geçici dosyalar yalnızca oyun tam yüklendikten sonra silinebilir".into());
    }

    let mut removed = Vec::new();
    let mut freed = 0u64;
    let mut take = |path: PathBuf| -> u64 {
        let size = dir_size(&path);
        if path.exists() {
            let _ = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            removed.push(path.to_string_lossy().to_string());
        }
        size
    };

    let downloads = downloads_dir(&root);
    for name in [
        crate::constants::PORT_ZIP_NAME,
        crate::constants::EXTRACTOR_ZIP_NAME,
    ] {
        freed += take(downloads.join(name));
    }
    if let Ok(entries) = std::fs::read_dir(&downloads) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map(|e| e == "part").unwrap_or(false) {
                freed += take(p);
            }
        }
    }
    freed += take(tools_dir(&root));

    Ok(CleanReport { removed, freed })
}

#[tauri::command]
pub fn shutdown_now() -> Result<(), String> {
    power::shutdown_pc().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn restart_now() -> Result<(), String> {
    power::restart_pc().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    let shared = app.state::<Shared>();
    if let Ok(root_guard) = shared.root.lock() {
        if let Some(root) = root_guard.as_ref() {
            if let Ok(state) = shared.state.lock() {
                let _ = state.save(&state_file(root));
            }
        }
    }
    // WebView2 hâlâ pencere sınıfına sahipken exit() → Chrome_WidgetWin_0 1412
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.destroy();
    }
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        app.exit(0);
    });
}

#[tauri::command]
pub fn ping() -> String {
    "pong".into()
}

#[tauri::command]
pub fn get_port_settings(app: AppHandle) -> Result<PortSettings, String> {
    let shared = app.state::<Shared>();
    let root = shared
        .root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "kurulum yolu seçilmedi".to_string())?;
    Ok(read_port_settings(&root))
}

#[tauri::command]
pub fn set_port_settings(app: AppHandle, settings: PortSettings) -> Result<PortSettings, String> {
    let shared = app.state::<Shared>();
    let root = shared
        .root
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "kurulum yolu seçilmedi".to_string())?;
    write_port_settings(&root, &settings).map_err(|e| e.to_string())?;
    Ok(read_port_settings(&root))
}
