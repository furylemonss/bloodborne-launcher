use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

pub struct DownloadOpts {
    pub url: &'static str,
    pub dest: PathBuf,
    pub expected_size: u64,
    pub step_id: &'static str,
}

pub async fn file_ready(path: &Path, expected_size: u64) -> bool {
    match tokio::fs::metadata(path).await {
        Ok(meta) if meta.is_file() => {
            let len = meta.len();
            len == expected_size || len.abs_diff(expected_size) < 4096
        }
        _ => false,
    }
}

pub async fn download_with_resume(
    opts: &DownloadOpts,
    paused: &Arc<AtomicBool>,
    cancel: &Arc<AtomicBool>,
    on_progress: &(dyn Fn(String, u64, u64, u64, u64) + Sync),
) -> Result<()> {
    if file_ready(&opts.dest, opts.expected_size).await {
        let len = tokio::fs::metadata(&opts.dest).await?.len();
        on_progress(opts.step_id.to_string(), len, len, 0, 0);
        return Ok(());
    }

    if tokio::fs::metadata(&opts.dest).await.is_ok() {
        let len = tokio::fs::metadata(&opts.dest).await?.len();
        if len.abs_diff(opts.expected_size) >= 4096 {
            tokio::fs::remove_file(&opts.dest).await.ok();
        }
    }

    let part = part_path(&opts.dest);
    let mut downloaded = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);

    if opts.expected_size > 0 && downloaded > opts.expected_size {
        tokio::fs::remove_file(&part).await.ok();
        downloaded = 0;
    }

    if opts.expected_size > 0 && downloaded == opts.expected_size {
        finalize(&part, &opts.dest).await?;
        on_progress(opts.step_id.to_string(), downloaded, opts.expected_size, 0, 0);
        return Ok(());
    }

    on_progress(opts.step_id.to_string(), downloaded, opts.expected_size, 0, 0);

    eprintln!(
        "[download] {} başlıyor · resume={} · hedef={:?}",
        opts.step_id, downloaded, opts.dest
    );

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .pool_idle_timeout(Duration::from_secs(30))
        .tcp_keepalive(Duration::from_secs(15))
        .http1_only()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) BloodborneInstaller/0.2")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    let mut attempt = 0u32;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(anyhow!("iptal edildi"));
        }
        if paused.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(150)).await;
            continue;
        }

        eprintln!(
            "[download] {} istek #{} · offset={}",
            opts.step_id,
            attempt + 1,
            downloaded
        );
        match try_once(
            &client,
            opts,
            &part,
            &mut downloaded,
            paused,
            cancel,
            on_progress,
        )
        .await
        {
            Ok(()) => {
                eprintln!("[download] {} bitti · {} bayt", opts.step_id, downloaded);
                return Ok(());
            }
            Err(err) => {
                if cancel.load(Ordering::SeqCst) {
                    return Err(anyhow!("iptal edildi"));
                }
                attempt += 1;
                eprintln!("[download] {} hata ({}): {}", opts.step_id, attempt, err);
                if attempt >= 8 {
                    return Err(err);
                }
                on_progress(
                    opts.step_id.to_string(),
                    downloaded,
                    opts.expected_size.max(downloaded),
                    0,
                    0,
                );
                let backoff = Duration::from_secs(1 + u64::from(attempt.min(6)));
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

async fn try_once(
    client: &reqwest::Client,
    opts: &DownloadOpts,
    part: &Path,
    downloaded: &mut u64,
    paused: &Arc<AtomicBool>,
    cancel: &Arc<AtomicBool>,
    on_progress: &(dyn Fn(String, u64, u64, u64, u64) + Sync),
) -> Result<()> {
    let mut req = client.get(opts.url);
    if *downloaded > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={}-", downloaded));
    }

    let resp = tokio::time::timeout(Duration::from_secs(60), req.send())
        .await
        .map_err(|_| anyhow!("sunucu yanıt vermedi (bağlantı zaman aşımı)"))?
        .context("istek gönderilemedi")?;

    let status = resp.status();
    if !(status.is_success() || status.as_u16() == 206) {
        return Err(anyhow!("HTTP {}", status));
    }

    let resumed_from = *downloaded;
    let is_partial = status.as_u16() == 206;
    let total = header_total(&resp, resumed_from, opts.expected_size);

    if !is_partial {
        *downloaded = 0;
    }

    let mut file = if *downloaded > 0 {
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(part)
            .await
            .context("part dosyası açılamadı")?
    } else {
        tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(part)
            .await
            .context("part dosyası açılamadı")?
    };

    on_progress(
        opts.step_id.to_string(),
        *downloaded,
        total.max(opts.expected_size),
        0,
        0,
    );

    let mut stream = resp.bytes_stream();
    let mut last_tick = Instant::now();
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let mut window_bytes = 0u64;
    let mut speed = 0f64;
    let mut avg_speed = 0f64;
    let mut session_bytes = 0u64;
    let session_start = Instant::now();
    let mut ready_eta = false;

    loop {
        if cancel.load(Ordering::SeqCst) {
            file.flush().await.ok();
            return Err(anyhow!("iptal edildi"));
        }
        while paused.load(Ordering::SeqCst) {
            if cancel.load(Ordering::SeqCst) {
                return Err(anyhow!("iptal edildi"));
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
            last_tick = Instant::now();
            last_emit = Instant::now();
        }

        let next = tokio::time::timeout(Duration::from_secs(45), stream.next()).await;
        let chunk = match next {
            Err(_) => {
                file.flush().await.ok();
                return Err(anyhow!("veri akışı durdu, yeniden bağlanılıyor"));
            }
            Ok(None) => break,
            Ok(Some(chunk)) => chunk.context("gövde okunamadı")?,
        };

        let write_res = file.write_all(&chunk).await;
        if let Err(e) = write_res {
            let os = e.raw_os_error();
            let disk = os == Some(112) /* ERROR_DISK_FULL */ || os == Some(28) /* ENOSPC */;
            return Err(if disk {
                anyhow!("Yetersiz disk. İndirme sırasında disk doldu (ENOSPC)")
            } else {
                anyhow!("part dosyasına yazılamadı: {e}")
            });
        }
        *downloaded += chunk.len() as u64;
        window_bytes += chunk.len() as u64;
        session_bytes += chunk.len() as u64;

        let now = Instant::now();
        let since_tick = now.duration_since(last_tick);
        if since_tick >= Duration::from_millis(700) {
            let inst = window_bytes as f64 / since_tick.as_secs_f64();
            // EMA: anlık sıçramaları yut, ~4–6 sn'lik yumuşak hız
            if speed <= 0.0 {
                speed = inst;
            } else {
                speed = 0.22 * inst + 0.78 * speed;
            }
            let elapsed = session_start.elapsed().as_secs_f64().max(0.001);
            avg_speed = session_bytes as f64 / elapsed;
            window_bytes = 0;
            last_tick = now;
            // Oturum 3 sn'yi geçince ETA aç; öncesinde 0 → UI "—" gösterir
            if elapsed >= 3.0 {
                ready_eta = true;
            }
        }

        if now.duration_since(last_emit) >= Duration::from_millis(350) || *downloaded >= total {
            // Raporlanan hız: uzun ortalamayı biraz öne al → ETA zıplamaz
            let reported = if avg_speed > 0.0 {
                (speed * 0.45 + avg_speed * 0.55).max(1.0)
            } else {
                speed.max(0.0)
            };
            let eta = if ready_eta && reported >= 1.0 && total > *downloaded {
                ((total - *downloaded) as f64 / reported).round() as u64
            } else {
                0
            };
            on_progress(
                opts.step_id.to_string(),
                *downloaded,
                total.max(*downloaded),
                reported as u64,
                eta,
            );
            last_emit = now;
        }
    }

    file.flush().await?;
    drop(file);

    if total > 0 && *downloaded + 1024 < total {
        return Err(anyhow!("eksik indirme: {downloaded}/{total}"));
    }

    finalize(part, &opts.dest).await?;
    on_progress(
        opts.step_id.to_string(),
        *downloaded,
        total.max(*downloaded),
        0,
        0,
    );
    Ok(())
}

fn header_total(resp: &reqwest::Response, downloaded: u64, expected: u64) -> u64 {
    if resp.status().as_u16() == 206 {
        if let Some(range) = resp.headers().get(reqwest::header::CONTENT_RANGE) {
            if let Ok(text) = range.to_str() {
                if let Some(total) = text.split('/').nth(1).and_then(|v| v.parse::<u64>().ok()) {
                    if total > 0 {
                        return total;
                    }
                }
            }
        }
        if let Some(len) = resp.content_length() {
            return downloaded + len;
        }
    } else if let Some(len) = resp.content_length() {
        return len;
    }
    if expected > 0 {
        expected
    } else {
        downloaded
    }
}

async fn finalize(part: &Path, dest: &Path) -> Result<()> {
    if dest.exists() {
        tokio::fs::remove_file(dest).await.ok();
    }
    tokio::fs::rename(part, dest).await.context("part yeniden adlandırılamadı")?;
    Ok(())
}

pub fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

pub fn free_space(path: &Path) -> u64 {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut best = path;
    while !best.exists() {
        match best.parent() {
            Some(parent) => best = parent,
            None => break,
        }
    }
    for disk in disks.list() {
        if best.starts_with(disk.mount_point()) {
            return disk.available_space();
        }
    }
    0
}
