use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::constants::{
    BASE_PKG_NAME, BASE_PKG_SIZE, BASE_PKG_URL, DOWNLOAD_BASE, DOWNLOAD_EXTRACTOR, DOWNLOAD_PORT,
    DOWNLOAD_UPDATE, EXTRACTOR_ZIP_NAME, EXTRACTOR_ZIP_SIZE, EXTRACTOR_ZIP_URL, PORT_ZIP_NAME,
    PORT_ZIP_SIZE, PORT_ZIP_URL, UPDATE_PKG_NAME, UPDATE_PKG_SIZE,
    UPDATE_PKG_URL,
};
use crate::download::{download_with_resume, file_ready, free_space, DownloadOpts};
use crate::extract::{
    drop_nested_title_dir, ensure_eboot_at_root, ensure_eboot_progress, ensure_patch_at_root,
    ensure_patch_progress, extractor_exe, extractor_zip_path, invalidate_misapplied_update_stamp,
    merge_dir_with_progress, normalize_port_zip_with_progress, port_exe, run_pkg_extractor,
    update_matches_game, verify_base_tree, verify_patch_tree, write_launcher_settings, extract_zip,
    extract_zip_with_progress,
};
use crate::notify;
use crate::state::{
    adopt_existing, downloads_dir, ensure_layout, game_dir, patch_dir, port_dir, state_file,
    tools_dir, Control, InstallerState, Status, StepStatus,
};

pub struct PipelineCtx {
    pub root: PathBuf,
    pub state: Arc<Mutex<InstallerState>>,
    pub control: Arc<Control>,
    pub app: tauri::AppHandle,
}

impl PipelineCtx {
    /// Emit'i sıralı kuyrukla tek worker'dan geçir: sıralar bozulmasın,
    /// extract callback'i Tauri IPC'de asla bloklanmasın.
    pub fn emit_progress(&self, value: serde_json::Value) {
        emit_worker().enqueue("pipeline://progress", value);
    }

    pub fn emit_error(&self, message: String) {
        emit_worker().enqueue(
            "pipeline://error",
            serde_json::json!({ "message": message }),
        );
    }

    pub fn emit_done(&self) {
        emit_worker().enqueue("pipeline://done", serde_json::json!({}));
    }

    fn save(&self) {
        if let Ok(state) = self.state.lock() {
            let _ = state.save(&state_file(&self.root));
        }
    }

    fn set_step(&self, index: usize, status: StepStatus, detail: Option<String>, percent: u8) {
        let snapshot = {
            if let Ok(mut state) = self.state.lock() {
                if let Some(step) = state.steps.get_mut(index) {
                    step.status = status;
                    step.detail = detail;
                    step.percent = percent;
                }
                state.current_step = index;
                state.clone()
            } else {
                return;
            }
        };
        // önce diske yaz, sonra emit — emit kitlense bile damga/durum kaybolmaz
        let _ = snapshot.save(&state_file(&self.root));
        self.emit_progress(serde_json::json!(snapshot));
    }

    fn set_status(&self, status: Status) {
        let snapshot = {
            if let Ok(mut state) = self.state.lock() {
                state.status = status;
                if status != Status::Error {
                    state.last_error = None;
                }
                state.clone()
            } else {
                return;
            }
        };
        let _ = snapshot.save(&state_file(&self.root));
        self.emit_progress(serde_json::json!(snapshot));
    }
}

struct EmitQueue {
    tx: Mutex<mpsc::Sender<(String, serde_json::Value)>>,
}

impl EmitQueue {
    fn enqueue(&self, event: &str, payload: serde_json::Value) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send((event.to_string(), payload));
        }
    }
}

static EMIT_QUEUE: std::sync::OnceLock<EmitQueue> = std::sync::OnceLock::new();

fn emit_worker() -> &'static EmitQueue {
    EMIT_QUEUE.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(String, serde_json::Value)>();
        std::thread::spawn(move || {
            // worker ilk çağrıda AppHandle'ı alamaz — channel tutucu tarafında
            // gerçek emit closure'ı start_pipeline'da register edilir
            while let Ok((event, payload)) = rx.recv() {
                if let Some(f) = emit_sink() {
                    f(&event, payload);
                }
            }
        });
        EmitQueue {
            tx: Mutex::new(tx),
        }
    })
}

static EMIT_SINK: std::sync::OnceLock<Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>> =
    std::sync::OnceLock::new();

fn emit_sink() -> Option<&'static Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>> {
    EMIT_SINK.get()
}

pub fn register_emit_sink(f: Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>) {
    let _ = EMIT_SINK.set(f);
}

pub async fn run_pipeline(ctx: Arc<PipelineCtx>) -> Result<(), String> {
    if ctx.control.running.swap(true, Ordering::SeqCst) {
        return Err("zaten çalışıyor".into());
    }
    ctx.control.reset_for_run();

    let result = run_inner(&ctx).await;

    ctx.control.running.store(false, Ordering::SeqCst);
    match result {
        Ok(()) => {
            ctx.set_status(Status::Ready);
            notify::installed(&ctx.app);
            ctx.emit_done();
            Ok(())
        }
        Err(err) => {
            if ctx.control.should_stop() {
                ctx.set_status(Status::Paused);
                Err("iptal edildi".into())
            } else if ctx.control.is_paused() {
                ctx.set_status(Status::Paused);
                Err("donduruldu".into())
            } else {
                if let Ok(mut state) = ctx.state.lock() {
                    state.last_error = Some(err.clone());
                    state.status = Status::Error;
                    // takılan adımı da hata olarak işaretle
                    let idx = state.current_step;
                    if let Some(step) = state.steps.get_mut(idx) {
                        if step.status == crate::state::StepStatus::Running {
                            step.status = crate::state::StepStatus::Error;
                        }
                    }
                    let snapshot = state.clone();
                    drop(state);
                    ctx.emit_progress(serde_json::json!(snapshot));
                }
                ctx.emit_error(err.clone());
                Err(err)
            }
        }
    }
}

async fn run_inner(ctx: &Arc<PipelineCtx>) -> Result<(), String> {
    let root = ctx.root.clone();
    ensure_layout(&root).map_err(|e| e.to_string())?;
    adopt_existing(&root);

    let free = free_space(&root);
    let required = crate::pipeline::required_free_bytes(&root);
    if free < required {
        let msg = format!(
            "Yetersiz disk. Gerekli ~{:.1} GB, bos: {:.1} GB",
            required as f64 / (1024.0 * 1024.0 * 1024.0),
            free as f64 / (1024.0 * 1024.0 * 1024.0)
        );
        return Err(msg);
    }

    let downloads = downloads_dir(&root);
    let tools = tools_dir(&root);
    let game = game_dir(&root);
    let patch = patch_dir(&root);
    let port = port_dir(&root);

    ctx.set_status(Status::Running);

    let progress_cb = {
        let ctx = ctx.clone();
        let last_save = Arc::new(std::sync::Mutex::new(Instant::now()));
        move |step_id: String, downloaded: u64, total: u64, speed: u64, eta: u64| {
            let percent = if total > 0 {
                ((downloaded as f64 / total as f64) * 100.0).floor().min(100.0) as u8
            } else {
                0u8
            };
            let detail = format!(
                "{} / {} · {}/s",
                format_bytes(downloaded),
                if total > 0 {
                    format_bytes(total)
                } else {
                    "?".into()
                },
                format_bytes(speed)
            );
            let mut should_save = false;
            if let Ok(mut state) = ctx.state.lock() {
                if let Some(step) = state.steps.iter_mut().find(|s| s.id == step_id) {
                    step.status = StepStatus::Running;
                    step.percent = percent;
                    step.detail = Some(detail.clone());
                }
                state.current_step = state
                    .steps
                    .iter()
                    .position(|s| s.id == step_id)
                    .unwrap_or(state.current_step);
                drop(state);
                if let Ok(mut stamp) = last_save.lock() {
                    if stamp.elapsed() >= std::time::Duration::from_secs(3) {
                        *stamp = Instant::now();
                        should_save = true;
                    }
                }
                if let Ok(snapshot) = ctx.state.lock() {
                    let snapshot = snapshot.clone();
                    ctx.emit_progress(serde_json::json!({
                        "download": {
                            "step_id": step_id,
                            "downloaded": downloaded,
                            "total": total,
                            "speed_bps": speed,
                            "eta_secs": eta
                        },
                        "status": snapshot.status,
                        "current_step": snapshot.current_step,
                        "steps": snapshot.steps,
                        "last_error": snapshot.last_error,
                        "completed": snapshot.completed,
                        "user_paused": snapshot.user_paused,
                        "root": snapshot.root,
                    }));
                }
            }
            if should_save {
                if let Ok(state) = ctx.state.lock() {
                    let _ = state.save(&state_file(&ctx.root));
                }
            }
        }
    };

    let paused = {
        let control = ctx.control.clone();
        Arc::new(std::sync::atomic::AtomicBool::new(
            control.is_paused(),
        ))
    };
    let cancel = {
        let control = ctx.control.clone();
        Arc::new(std::sync::atomic::AtomicBool::new(
            control.should_stop(),
        ))
    };
    let control_for_hooks = ctx.control.clone();
    let paused_hook = paused.clone();
    let cancel_hook = cancel.clone();
    spawn_step_watchdog(ctx.clone(), cancel.clone());
    let sync_task = tokio::spawn(async move {
        loop {
            paused_hook.store(control_for_hooks.is_paused(), Ordering::SeqCst);
            cancel_hook.store(control_for_hooks.should_stop(), Ordering::SeqCst);
            if control_for_hooks.should_stop() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });

    let result = pipeline_steps(
        ctx,
        &downloads,
        &tools,
        &game,
        &patch,
        &port,
        &paused,
        &cancel,
        &progress_cb,
    )
    .await;

    cancel.store(true, Ordering::SeqCst);
    let _ = sync_task.await;
    result
}

/// %100'de takılı kalan adım: exe yerindeyse UI kendi kendine ilerlesin.
fn spawn_step_watchdog(ctx: Arc<PipelineCtx>, cancel: Arc<std::sync::atomic::AtomicBool>) {
    std::thread::spawn(move || {
        let mut last = (0u8, String::new());
        let mut same_since = Instant::now();
        while !cancel.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let snap = {
                let Ok(state) = ctx.state.lock() else { continue };
                (
                    state.current_step,
                    state.steps.get(7).cloned(),
                    matches!(
                        state.status,
                        Status::Running | Status::Extracting
                    ),
                )
            };
            let (idx, step, active) = snap;
            if !active || idx != 7 {
                last = (0, String::new());
                same_since = Instant::now();
                continue;
            }
            let Some(step) = step else { continue };
            if step.status != StepStatus::Running || step.percent < 100 {
                last = (step.percent, step.detail.clone().unwrap_or_default());
                same_since = Instant::now();
                continue;
            }
            let detail = step.detail.clone().unwrap_or_default();
            if (step.percent, detail.clone()) != last {
                last = (step.percent, detail);
                same_since = Instant::now();
                continue;
            }
            if same_since.elapsed() < std::time::Duration::from_secs(12) {
                continue;
            }
            let port = port_dir(&ctx.root);
            if port_exe(&port).exists() {
                eprintln!("[extract] watchdog: %100 takıldı ama exe var — adım bitiriliyor");
                let _ = std::fs::write(port.join(".bb-port-ok"), b"1");
                ctx.set_step(7, StepStatus::Done, Some("tamam".into()), 100);
            } else if detail != "exe aranıyor" {
                eprintln!("[extract] watchdog: %100 takıldı — exe aranıyor fazına yazılıyor");
                ctx.set_step(7, StepStatus::Running, Some("exe aranıyor".into()), 100);
            }
            same_since = Instant::now();
        }
    });
}

async fn pipeline_steps(
    ctx: &Arc<PipelineCtx>,
    downloads: &Path,
    tools: &Path,
    game: &Path,
    patch: &Path,
    port: &Path,
    paused: &Arc<std::sync::atomic::AtomicBool>,
    cancel: &Arc<std::sync::atomic::AtomicBool>,
    progress_cb: &(dyn Fn(String, u64, u64, u64, u64) + Sync),
) -> Result<(), String> {
    let checks: Vec<(&str, usize, PathBuf, u64)> = vec![
        (
            DOWNLOAD_BASE,
            0,
            downloads.join(BASE_PKG_NAME),
            BASE_PKG_SIZE,
        ),
        (
            DOWNLOAD_UPDATE,
            1,
            downloads.join(UPDATE_PKG_NAME),
            UPDATE_PKG_SIZE,
        ),
        (DOWNLOAD_PORT, 2, downloads.join(PORT_ZIP_NAME), PORT_ZIP_SIZE),
        (
            DOWNLOAD_EXTRACTOR,
            3,
            downloads.join(EXTRACTOR_ZIP_NAME),
            EXTRACTOR_ZIP_SIZE,
        ),
    ];

    for (step_id, index, dest, size) in checks {
        if ctx.control.should_stop() {
            return Err("iptal edildi".into());
        }
        if ctx.control.is_paused() {
            return Err("donduruldu".into());
        }

        if file_ready(&dest, size).await {
            ctx.set_step(index, StepStatus::Skipped, Some("zaten mevcut".into()), 100);
            continue;
        }

        ctx.set_step(index, StepStatus::Running, Some("başlıyor".into()), 0);
        let opts = DownloadOpts {
            url: match step_id {
                DOWNLOAD_BASE => BASE_PKG_URL,
                DOWNLOAD_UPDATE => UPDATE_PKG_URL,
                DOWNLOAD_PORT => PORT_ZIP_URL,
                _ => EXTRACTOR_ZIP_URL,
            },
            dest: dest.clone(),
            expected_size: size,
            step_id,
        };
        let file_label = match step_id {
            DOWNLOAD_BASE => "oyun PKG",
            DOWNLOAD_UPDATE => "update PKG",
            DOWNLOAD_PORT => "port zip",
            _ => "PkgExtractor",
        };
        download_with_resume(&opts, paused, cancel, &progress_cb)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("yetersiz") || msg.contains("disk") || msg.contains("ENOSPC") {
                    format!("Yetersiz disk. {file_label} indirilemedi, bos alan yetmiyor")
                } else {
                    format!("{file_label} indirilemedi: {msg}")
                }
            })?;
        ctx.set_step(index, StepStatus::Done, None, 100);
    }

    if ctx.control.should_stop() {
        return Err("iptal edildi".into());
    }

    let eboot = game.join("eboot.bin");
    let base_stamp = game.join(".bb-base-ok");
    let update_stamp = game.join(".bb-update-ok");
    let game_owned = game.to_path_buf();
    let patch_owned = patch.to_path_buf();

    // Çıkartma o an tamamlanmalı: damga yoksa (kapanıp açılınca da) game/ silinir
    if clean_incomplete_extract(game) {
        eprintln!("[extract] eksik çıkartma temizlendi, pkg'den baştan çıkartılacak");
    }

    let base_ok = eboot.exists() && (base_stamp.exists() || update_stamp.exists());
    // Damga yetmez: eboot boyutu patch ile aynıysa update gerçekten game üstüne binmiştir
    let update_ok = base_ok && update_stamp.exists() && update_matches_game(game, patch);
    if update_stamp.exists() && !update_ok {
        invalidate_misapplied_update_stamp(game, patch);
        eprintln!("[extract] update uygulanmamış — merge yeniden yapılacak");
    }

    if base_ok && update_ok {
        if !base_stamp.exists() {
            let _ = std::fs::write(&base_stamp, b"1");
        }
        ctx.set_step(4, StepStatus::Skipped, Some("eboot.bin mevcut".into()), 100);
        ctx.set_step(5, StepStatus::Skipped, Some("update sonrasi".into()), 100);
        ctx.set_step(6, StepStatus::Skipped, Some("update uygulanmis".into()), 100);
    } else {
        // çıkartma / merge disk'i yiyor — başlamadan tekrar bak
        let free = free_space(game);
        let required = required_free_bytes(&ctx.root);
        if free < required {
            return Err(format!(
                "Yetersiz disk. Gerekli ~{:.1} GB, bos: {:.1} GB",
                required as f64 / (1024.0 * 1024.0 * 1024.0),
                free as f64 / (1024.0 * 1024.0 * 1024.0)
            ));
        }
        ctx.set_status(Status::Extracting);
        let exe = extractor_exe(tools);
        if !exe.exists() {
            let zip = extractor_zip_path(downloads);
            extract_zip(&zip, tools).map_err(|e| e.to_string())?;
        }

        if !base_ok {
            ctx.set_step(4, StepStatus::Running, Some("başlıyor".into()), 0);
            // Zaten tam ağaç varsa (downloads\CUSA03173) PKG'den tekrar çıkarma
            let pre_ok = verify_base_tree(&game).is_ok();
            if pre_ok {
                eprintln!("[extract] oyun ağacı zaten tam — çıkartma atlanıyor");
                let _ = std::fs::write(&base_stamp, b"1");
                ctx.set_step(4, StepStatus::Done, Some("tamam".into()), 100);
            } else {
                // extractor `out/CUSA03173` yazar — out = downloads olursa
                // downloads/CUSA03173 oluşur (iç içe CUSA03173 olmaz).
                // Eksik eski hedefi temizle ki tekrar nested oluşmasın.
                if game.exists() {
                    eprintln!("[extract] eksik hedef temizleniyor: {}", game.display());
                    let _ = std::fs::remove_dir_all(&game);
                }
                {
                    let cb_ctx = ctx.clone();
                    let exe_c = exe.clone();
                    let pkg_c = downloads.join(BASE_PKG_NAME);
                    let out_c = downloads.to_path_buf();
                    let join = tokio::task::spawn_blocking(move || {
                        run_pkg_extractor(&exe_c, &pkg_c, &out_c, &move |pct, detail| {
                            let d = if detail.is_empty() {
                                if pct >= 100 {
                                    "tamam".to_string()
                                } else {
                                    format!("{pct}%")
                                }
                            } else {
                                detail
                            };
                            cb_ctx.set_step(4, StepStatus::Running, Some(d), pct.min(99));
                        })
                    })
                    .await
                    .map_err(|e| e.to_string())?;
                    join.map_err(|e| e.to_string())?;
                }
                eprintln!("[extract] base bitti — eboot aranıyor");
                ctx.set_step(4, StepStatus::Running, Some("yerleştiriliyor".into()), 99);
                {
                    let cb_ctx = ctx.clone();
                    let game_c = game_owned.clone();
                    tokio::task::spawn_blocking(move || {
                        ensure_eboot_progress(&game_c, Some(&move |pct, detail| {
                            cb_ctx.set_step(
                                4,
                                StepStatus::Running,
                                Some(format!("yerleştiriliyor · {detail}")),
                                pct.min(99),
                            );
                        }))
                    })
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e: anyhow::Error| e.to_string())?;
                }
                drop_nested_title_dir(game, "CUSA03173");
                ctx.set_step(4, StepStatus::Running, Some("doğrulanıyor".into()), 99);
                verify_base_tree(game).map_err(|e| e.to_string())?;
                eprintln!("[extract] base doğrulandı — damga yazılıyor");
                let _ = std::fs::write(&base_stamp, b"1");
                ctx.set_step(4, StepStatus::Done, Some("tamam".into()), 100);
                eprintln!("[extract] base damgası yazıldı, update aşamasına geçiliyor");
            }
        } else {
            if !base_stamp.exists() {
                let _ = std::fs::write(&base_stamp, b"1");
            }
            ctx.set_step(4, StepStatus::Skipped, Some("eboot.bin mevcut".into()), 100);
        }

        if ctx.control.should_stop() {
            return Err("iptal edildi".into());
        }

        // Update yoksa (damga yok) PATCH'i silıp baştan uygula — game'e dokunma
        ctx.set_step(5, StepStatus::Running, Some("başlıyor".into()), 0);
        let patch_pre_ok = verify_patch_tree(&patch).is_ok();
        if !patch_pre_ok {
            let _ = std::fs::remove_dir_all(&patch);
            let _ = std::fs::create_dir_all(&patch);
        {
            let cb_ctx = ctx.clone();
            let exe_c = exe.clone();
            let pkg_c = downloads.join(UPDATE_PKG_NAME);
            let patch_c = patch_owned.clone();
            let join = tokio::task::spawn_blocking(move || {
                run_pkg_extractor(&exe_c, &pkg_c, &patch_c, &move |pct, detail| {
                    let d = if detail.is_empty() {
                        if pct >= 100 {
                            "tamam".to_string()
                        } else {
                            format!("{pct}%")
                        }
                    } else {
                        detail
                    };
                    cb_ctx.set_step(5, StepStatus::Running, Some(d), pct.min(99));
                })
            })
            .await
            .map_err(|e| e.to_string())?;
            join.map_err(|e| e.to_string())?;
            }
        } else {
            eprintln!("[extract] update ağacı zaten tam — çıkartma atlanıyor");
        }
        // PKG içeriği iç içe klasörde kalır; merge öncesi köke indir (rename)
        ctx.set_step(5, StepStatus::Running, Some("yerleştiriliyor".into()), 99);
        {
            let cb_ctx = ctx.clone();
            let patch_c = patch_owned.clone();
            tokio::task::spawn_blocking(move || {
                ensure_patch_progress(&patch_c, Some(&move |pct, detail| {
                    cb_ctx.set_step(
                        5,
                        StepStatus::Running,
                        Some(format!("yerleştiriliyor · {detail}")),
                        pct.min(99),
                    );
                }))
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e: anyhow::Error| e.to_string())?;
        }
        drop_nested_title_dir(&patch, "CUSA03173-patch");
        ctx.set_step(5, StepStatus::Running, Some("doğrulanıyor".into()), 99);
        verify_patch_tree(&patch).map_err(|e| e.to_string())?;
        ctx.set_step(5, StepStatus::Done, None, 100);

        if ctx.control.should_stop() {
            return Err("iptal edildi".into());
        }

        ctx.set_step(6, StepStatus::Running, Some("başlıyor".into()), 0);
        {
            let cb_ctx = ctx.clone();
            let patch_c = patch_owned.clone();
            let game_c = game_owned.clone();
            let join = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                ensure_patch_at_root(&patch_c)?;
                merge_dir_with_progress(
                    &patch_c,
                    &game_c,
                    Some(&move |pct, detail| {
                        let d = if detail.is_empty() {
                            if pct >= 100 {
                                "tamam".to_string()
                            } else {
                                format!("{pct}%")
                            }
                        } else {
                            detail
                        };
                        cb_ctx.set_step(6, StepStatus::Running, Some(d), pct);
                    }),
                )
            })
            .await
            .map_err(|e| e.to_string())?;
            join.map_err(|e| e.to_string())?;
        }
        // eboot gerçekten üstüne binmeden damga yazma
        if !update_matches_game(&game, &patch) {
            return Err(
                "update merge sonrası game/eboot.bin patch ile uyuşmuyor — patch uygulanmadı"
                    .into(),
            );
        }
        // önceki hatalı merge artığı: patch klasörü oyun köküne kopyalanmıştı
        let _ = std::fs::remove_dir_all(game.join("CUSA03173-patch"));
        let _ = std::fs::remove_dir_all(game.join("CUSA03173").join("CUSA03173-patch"));
        let _ = std::fs::write(&update_stamp, b"1");
        eprintln!(
            "[extract] update uygulandı: game eboot {} byte",
            std::fs::metadata(&eboot).map(|m| m.len()).unwrap_or(0)
        );
        ctx.set_step(6, StepStatus::Done, None, 100);
    }

    if ctx.control.should_stop() {
        return Err("iptal edildi".into());
    }

    if port_exe(port).exists() {
        let stamp = port.join(".bb-port-ok");
        if !stamp.exists() {
            let _ = std::fs::write(&stamp, b"1");
        }
        ctx.set_step(7, StepStatus::Skipped, Some("port mevcut".into()), 100);
    } else {
        ctx.set_status(Status::Extracting);
        ctx.set_step(7, StepStatus::Running, Some("başlıyor".into()), 0);

        // Eski tmp/rename yolu Windows'ta kilitli dizinde takılıyordu.
        // Port doğrudan hedefe açılır; yarım kalırsa silinip yeniden denenir.
        let stale_tmp = port.with_extension("tmp-extract");
        if stale_tmp.exists() {
            eprintln!("[extract] eski tmp temizleniyor: {}", stale_tmp.display());
            let _ = std::fs::remove_dir_all(&stale_tmp);
        }

        let stamp = port.join(".bb-port-ok");
        if port.exists() && !stamp.exists() && !port_exe(port).exists() {
            // önceki yarım normalize/rename artığı — temiz başla
            eprintln!("[extract] yarım port temizleniyor: {}", port.display());
            let _ = std::fs::remove_dir_all(port);
            std::fs::create_dir_all(port).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir_all(port).map_err(|e| e.to_string())?;

        {
            let cb_ctx = ctx.clone();
            let zip_c = downloads.join(PORT_ZIP_NAME);
            let port_c = port.to_path_buf();
            let join = tokio::task::spawn_blocking(move || {
                extract_zip_with_progress(
                    &zip_c,
                    &port_c,
                    Some(&move |pct, detail| {
                        let d = if detail.is_empty() {
                            if pct >= 100 {
                                "tamam".to_string()
                            } else {
                                format!("{pct}%")
                            }
                        } else {
                            detail
                        };
                        cb_ctx.set_step(7, StepStatus::Running, Some(d), pct);
                    }),
                )
            })
            .await
            .map_err(|e| e.to_string())?;
            join.map_err(|e| e.to_string())?;
        }

        ctx.set_step(7, StepStatus::Running, Some("düzenleniyor".into()), 100);
        {
            let cb_ctx = ctx.clone();
            let port_c = port.to_path_buf();
            let join = tokio::task::spawn_blocking(move || {
                normalize_port_zip_with_progress(
                    &port_c,
                    Some(&move |pct, detail| {
                        let d = if detail.is_empty() {
                            if pct >= 100 {
                                "tamam".to_string()
                            } else {
                                format!("{pct}%")
                            }
                        } else if detail == "exe aranıyor" || detail == "exe bulundu" {
                            detail
                        } else {
                            format!("düzenleniyor · {detail}")
                        };
                        cb_ctx.set_step(7, StepStatus::Running, Some(d), pct);
                    }),
                )
            });
            // normalize asılı kalırsa UI sonsuza kadar 420/420'de donmasın
            match tokio::time::timeout(std::time::Duration::from_secs(180), join).await {
                Ok(Ok(join)) => join.map_err(|e| e.to_string())?,
                Ok(Err(e)) => return Err(e.to_string()),
                Err(_) => {
                    eprintln!("[extract] normalize 180sn'de dönmedi — timeout");
                    if !port_exe(port).exists() {
                        return Err("port düzenleme (normalize) takıldı".into());
                    }
                    eprintln!("[extract] timeout ama exe var — devam");
                }
            }
        }

        // set_step ÖNCE — eprintln konsol borusunda asılı kalsa bile UI ilerlesin
        ctx.set_step(7, StepStatus::Running, Some("exe aranıyor".into()), 100);
        {
            let listing: Vec<String> = std::fs::read_dir(port)
                .map(|rd| {
                    rd.flatten()
                        .take(12)
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default();
            eprintln!(
                "[extract] normalize döndü, exe kontrolü: {} (içindekiler: {})",
                port.display(),
                if listing.is_empty() {
                    "boş".into()
                } else {
                    listing.join(", ")
                }
            );
        }
        if !port_exe(port).exists() {
            let listing: Vec<String> = std::fs::read_dir(port)
                .map(|rd| {
                    rd.flatten()
                        .take(12)
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default();
            return Err(format!(
                "port çıkartıldı ama Bloodborne.exe bulunamadı: {} (içindekiler: {})",
                port.display(),
                if listing.is_empty() {
                    "boş".into()
                } else {
                    listing.join(", ")
                }
            ));
        }
        let _ = std::fs::write(&stamp, b"1");
        eprintln!("[extract] port hazır: {}", port_exe(port).display());
        ctx.set_step(7, StepStatus::Done, None, 100);
    }

    if ctx.control.should_stop() {
        return Err("iptal edildi".into());
    }

    ctx.set_step(8, StepStatus::Running, Some("yazılıyor".into()), 40);
    write_launcher_settings(game).map_err(|e| e.to_string())?;
    ctx.set_step(8, StepStatus::Done, None, 100);

    if let Ok(mut state) = ctx.state.lock() {
        state.completed = true;
        state.user_paused = false;
        state.last_error = None;
        let snapshot = state.clone();
        drop(state);
        ctx.emit_progress(serde_json::json!(snapshot));
    }
    ctx.save();
    Ok(())
}

pub fn detect_ready(root: &Path) -> bool {
    let game = game_dir(root);
    let patch = patch_dir(root);
    let eboot_ok = game.join("eboot.bin").exists()
        && (game.join(".bb-base-ok").exists() || game.join(".bb-update-ok").exists());
    eboot_ok
        && game.join(".bb-update-ok").exists()
        && update_matches_game(&game, &patch)
        && port_exe(root).exists()
}

/// Kalan iş için gereken boş alan.
/// PKG zaten indirildiyse o ~31 GB'ı tekrar isteme; çıkarılacak kısım + eksik indirmeler.
pub fn required_free_bytes(root: &Path) -> u64 {
    let downloads = downloads_dir(root);
    let game = game_dir(root);
    let port = port_dir(root);
    let patch = patch_dir(root);
    let tools = crate::state::tools_dir(root);

    let mut need = 0u64;

    let base_pkg_ok = file_ready_sync(&downloads.join(BASE_PKG_NAME), BASE_PKG_SIZE);
    let update_pkg_ok = file_ready_sync(&downloads.join(UPDATE_PKG_NAME), UPDATE_PKG_SIZE);
    let port_zip_ok = file_ready_sync(&downloads.join(PORT_ZIP_NAME), PORT_ZIP_SIZE);
    let extractor_zip_ok =
        file_ready_sync(&downloads.join(EXTRACTOR_ZIP_NAME), EXTRACTOR_ZIP_SIZE);

    // base: PKG yoksa indir + zaten varsa çıkartma payı
    if !base_pkg_ok {
        need += BASE_PKG_SIZE;
    }
    let base_done = base_extract_complete(&game);
    if !base_done {
        let already = crate::extract::tree_bytes(&game);
        need += BASE_PKG_SIZE.saturating_sub(already.min(BASE_PKG_SIZE));
        // hiç başlamadıysa tam çıkartma; kısmi ise eksik kısım
        if already == 0 {
            // saturating_sub zaten BASE_PKG_SIZE verdi
        }
    }

    if !update_pkg_ok {
        need += UPDATE_PKG_SIZE;
    }
    let update_ok = base_done && game.join(".bb-update-ok").exists() && update_matches_game(&game, &patch);
    if !update_ok {
        // patch/ klasörüne ayrıca çıkarılır
        need += UPDATE_PKG_SIZE;
    }

    if !port_zip_ok {
        need += PORT_ZIP_SIZE;
    }
    if !port_exe(&port).exists() {
        // zip ~122 MB, açılmış hali ~300 MB
        need += 400 * 1024 * 1024;
    }

    if !extractor_zip_ok && !crate::extract::extractor_exe(&tools).exists() {
        need += EXTRACTOR_ZIP_SIZE.max(64 * 1024 * 1024);
    }

    // kopya/merge + Defender için pay
    need += 3 * 1024 * 1024 * 1024;

    // asla 65 GB'ın altına düşürme (boş diskte hâlâ tam paket gerekir)
    // ama zaten mevcut PKG ile hesaplanan değer bunu sağlamamalı — clamp değil floor küçük tut
    need
}

pub fn summarize_steps(state: &mut InstallerState, root: &Path) {
    let downloads = downloads_dir(root);
    let game = game_dir(root);
    let patch = patch_dir(root);
    let port = port_dir(root);
    // Yarım çıkartma: program yeniden açıldıysa game/ silinir
    clean_incomplete_extract(&game);
    // Damga var ama eboot patch'e binmemişse — düşür, yeniden uygulansın
    invalidate_misapplied_update_stamp(&game, &patch);
    let ready = [
        (0, downloads.join(BASE_PKG_NAME), BASE_PKG_SIZE),
        (1, downloads.join(UPDATE_PKG_NAME), UPDATE_PKG_SIZE),
        (2, downloads.join(PORT_ZIP_NAME), PORT_ZIP_SIZE),
        (3, downloads.join(EXTRACTOR_ZIP_NAME), EXTRACTOR_ZIP_SIZE),
    ];
    for (index, path, size) in ready {
        if file_ready_sync(&path, size) {
            if let Some(step) = state.steps.get_mut(index) {
                step.status = StepStatus::Skipped;
                step.detail = Some("zaten mevcut".into());
                step.percent = 100;
            }
        }
    }
    let base_done = base_extract_complete(&game);
    let update_done = game.join(".bb-update-ok").exists() && update_matches_game(&game, &patch);
    if base_done {
        if let Some(step) = state.steps.get_mut(4) {
            step.status = StepStatus::Skipped;
            step.detail = Some("tamam".into());
            step.percent = 100;
        }
    }
    if update_done {
        for index in [5, 6] {
            if let Some(step) = state.steps.get_mut(index) {
                step.status = StepStatus::Skipped;
                step.detail = Some("tamam".into());
                step.percent = 100;
            }
        }
    }
    if port_exe(&port).exists() {
        if let Some(step) = state.steps.get_mut(7) {
            step.status = StepStatus::Skipped;
            step.detail = Some("port mevcut".into());
            step.percent = 100;
        }
    }
    if detect_ready(root) {
        state.completed = true;
        state.status = Status::Ready;
        if let Some(step) = state.steps.get_mut(8) {
            step.status = StepStatus::Done;
            step.percent = 100;
        }
    }
}

fn file_ready_sync(path: &Path, expected: u64) -> bool {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => {
            let len = meta.len();
            len == expected || len.abs_diff(expected) < 4096
        }
        _ => false,
    }
}

fn dir_has_entries(path: &Path) -> bool {
    match std::fs::read_dir(path) {
        Ok(mut entries) => entries.next().is_some(),
        Err(_) => false,
    }
}

/// base çıkartma tamamlanmış mı? eboot + damga; damga yoksa manifest doğrulaması.
fn base_extract_complete(game: &Path) -> bool {
    if !game.join("eboot.bin").exists() {
        return false;
    }
    if game.join(".bb-base-ok").exists() || game.join(".bb-update-ok").exists() {
        return true;
    }
    verify_base_tree(game).is_ok()
}

/// Yarım çıkartmayı temizle (damga yoksa ve manifest tutmuyorsa silinir).
/// true = bir şey silindi.
pub fn clean_incomplete_extract(game: &Path) -> bool {
    if !dir_has_entries(game) {
        return false;
    }
    if base_extract_complete(game) {
        // eski kurulum: update damgası varsa base damgasını da yaz
        if !game.join(".bb-base-ok").exists() {
            let _ = std::fs::write(game.join(".bb-base-ok"), b"1");
        }
        return false;
    }

    // Crash/rebuild recovery: çıkartma bitmiş ama damga yazılmadan process
    // ölmüş olabilir. Manifest tutuyorsa silme — damga yaz, devam et.
    if verify_base_tree(game).is_ok() {
        let _ = std::fs::write(game.join(".bb-base-ok"), b"1");
        eprintln!("[extract] manifest doğrulandı — silinmedi, damga yazıldı");
        return false;
    }

    const MIN_BASE: u64 = 20 * 1024 * 1024 * 1024;
    let bytes = crate::extract::tree_bytes(game);
    if bytes >= MIN_BASE {
        eprintln!(
            "[extract] damgasız ama büyük ağaç ({bytes} bayt) — flatten deneniyor"
        );
        let _ = ensure_eboot_at_root(game);
        let _ = crate::extract::drop_nested_title_dir(game, "CUSA03173");
        if verify_base_tree(game).is_ok() {
            let _ = std::fs::write(game.join(".bb-base-ok"), b"1");
            eprintln!("[extract] flatten sonrası manifest tuttu — silinmedi");
            return false;
        }
    }

    eprintln!(
        "[extract] eksik/bozuk çıkartma, siliniyor: {}",
        game.display()
    );
    if std::fs::remove_dir_all(game).is_err() {
        return false;
    }
    let _ = std::fs::create_dir_all(game);
    true
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut i = 0;
    while value >= 1024.0 && i < UNITS.len() - 1 {
        value /= 1024.0;
        i += 1;
    }
    if i == 0 || value >= 100.0 {
        format!("{} {}", bytes, UNITS[i])
    } else {
        format!("{:.1} {}", value, UNITS[i])
    }
}
