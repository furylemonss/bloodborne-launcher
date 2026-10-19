use anyhow::{anyhow, Context, Result};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::constants::{
    EXTRACTOR_EXE_REL, EXTRACTOR_ZIP_NAME, BASE_EBOOT_SIZE, PATCH_EBOOT_SIZE,
};

pub fn extractor_exe(tools: &Path) -> PathBuf {
    tools.join(EXTRACTOR_EXE_REL)
}

/// "Extracting file 531 of 29759 to \"...\"" ? (531, 29759)
fn parse_extract_line(line: &str) -> Option<(u64, u64)> {
    let idx = line.find(" of ")?;
    let before = &line[..idx];
    let after = &line[idx + 4..];
    let cur_s = before
        .rsplit(|c: char| !c.is_ascii_digit())
        .find(|s| !s.is_empty())?;
    let tot_s = after
        .split(|c: char| !c.is_ascii_digit())
        .find(|s| !s.is_empty())?;
    let cur: u64 = cur_s.parse().ok()?;
    let tot: u64 = tot_s.parse().ok()?;
    if tot == 0 || cur > tot + 100 {
        return None;
    }
    Some((cur, tot))
}

fn count_files_tree(dir: &Path) -> u64 {
    let mut n = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else { continue };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                n += 1;
            }
        }
    }
    n
}

fn find_named(dir: &Path, name: &str) -> Option<PathBuf> {
    // symlink/junction d?ng?s?ne kar?? iteratif arama + canonical seen set
    let mut stack = vec![dir.to_path_buf()];
    let mut seen = std::collections::HashSet::new();
    let mut steps = 0u32;
    while let Some(d) = stack.pop() {
        steps += 1;
        if steps > 20_000 {
            break;
        }
        if let Ok(canon) = std::fs::canonicalize(&d) {
            if !seen.insert(canon) {
                continue;
            }
        }
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(md) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if md.file_type().is_symlink() {
                continue;
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                // FILE_ATTRIBUTE_REPARSE_POINT ? junction da olsa d?nme
                if md.file_attributes() & 0x400 != 0 {
                    continue;
                }
            }
            if md.is_dir() {
                stack.push(path);
            } else if path.file_name().map(|f| f == name).unwrap_or(false) {
                return Some(path);
            }
        }
    }
    None
}

/// Aynı boyutlu dosya zaten varsa sil, yoksa rename (aynı volume → saniyeler).
fn move_one(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        let same = match (std::fs::metadata(src), std::fs::metadata(dst)) {
            (Ok(a), Ok(b)) if a.is_file() && b.is_file() && a.len() == b.len() => true,
            _ => false,
        };
        if same {
            let _ = std::fs::remove_file(src);
            return Ok(());
        }
        if dst.is_dir() {
            let _ = std::fs::remove_dir_all(dst);
        } else {
            let _ = std::fs::remove_file(dst);
        }
    }
    if let Some(p) = dst.parent() {
        std::fs::create_dir_all(p)?;
    }
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            // kilitli/k farklı volume: kopyala, sonra kaydı sil
            if src.is_dir() {
                std::fs::create_dir_all(dst)?;
                for entry in std::fs::read_dir(src)? {
                    let e = entry?;
                    move_one(&e.path(), &dst.join(e.file_name()))?;
                }
                let _ = std::fs::remove_dir_all(src);
                Ok(())
            } else {
                std::fs::copy(src, dst).with_context(|| {
                    format!("tasinamadi {} -> {}", src.display(), dst.display())
                })?;
                let _ = std::fs::remove_file(src);
                Ok(())
            }
        }
    }
}

fn flatten_move_rec(
    from: &Path,
    to: &Path,
    done: &mut u64,
    total: u64,
    last_pct: &mut u8,
    cb: Option<&(dyn Fn(u8, String) + Sync)>,
) -> Result<()> {
    std::fs::create_dir_all(to)?;
    let entries: Vec<_> = std::fs::read_dir(from)?.flatten().collect();
    for entry in entries {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            if dst.is_dir() {
                flatten_move_rec(&src, &dst, done, total, last_pct, cb)?;
                let _ = std::fs::remove_dir(&src);
            } else {
                let before = *done;
                move_one(&src, &dst)?;
                let moved = count_files(&dst).max(1);
                let _ = before;
                *done += moved;
            }
        } else {
            move_one(&src, &dst)?;
            *done += 1;
        }
        if let Some(cb) = cb {
            let pct = ((*done as f64 / total as f64) * 100.0).floor().min(100.0) as u8;
            if pct != *last_pct || *done == total {
                *last_pct = pct;
                cb(pct.min(99), format!("{done} / {total}"));
            }
        }
    }
    Ok(())
}

/// Nested klasör içeriğini root'a **taşır** (rename; kopya değil).
pub fn flatten_by_move(
    nested: &Path,
    root: &Path,
    on_progress: Option<&(dyn Fn(u8, String) + Sync)>,
) -> Result<()> {
    if !nested.is_dir() || nested == root {
        return Ok(());
    }
    let total = count_files(nested).max(1);
    eprintln!(
        "[extract] flatten(rename): {} -> {} ({total} dosya)",
        nested.display(),
        root.display()
    );
    let mut done = 0u64;
    let mut last_pct = 255u8;
    flatten_move_rec(nested, root, &mut done, total, &mut last_pct, on_progress)?;
    if let Some(cb) = on_progress {
        cb(100, format!("{total} / {total}"));
    }
    let _ = std::fs::remove_dir(nested);
    Ok(())
}

/// `root/CUSA03173` ve `root/CUSA03173-patch` gibi title klasörlerini köke indir.
pub fn flatten_nested_titles(
    root: &Path,
    on_progress: Option<&(dyn Fn(u8, String) + Sync)>,
) {
    for name in ["CUSA03173", "CUSA03173-patch"] {
        let nested = root.join(name);
        if !nested.is_dir() {
            continue;
        }
        let has_payload = nested.join("eboot.bin").exists()
            || nested.join("sce_sys").is_dir()
            || find_named(&nested, "eboot.bin").is_some();
        if !has_payload {
            continue;
        }
        // kökte tam eboot yoksa veya nested hâlâ doluysa taşı
        eprintln!("[extract] nested title: {}", nested.display());
        let _ = flatten_by_move(&nested, root, on_progress);
    }
}

/// eboot.bin kökte yoksa alt klasörden bulup üstteki köke indir.
/// Nested title klasörleri önce **rename** ile taşınır (31GB kopya değil).
pub fn ensure_eboot_at_root(game: &Path) -> Result<PathBuf> {
    ensure_eboot_progress(game, None)
}

pub fn ensure_eboot_progress(
    game: &Path,
    on_progress: Option<&(dyn Fn(u8, String) + Sync)>,
) -> Result<PathBuf> {
    flatten_nested_titles(game, on_progress);
    let direct = game.join("eboot.bin");
    if direct.exists() {
        return Ok(direct);
    }
    if let Some(found) = find_named(game, "eboot.bin") {
        let parent = found.parent().unwrap_or(game).to_path_buf();
        if parent != game && parent.starts_with(game) {
            eprintln!("[extract] eboot alt klasörde - flatten: {}", found.display());
            flatten_by_move(&parent, game, on_progress)?;
            let _ = std::fs::remove_dir_all(&parent);
        }
        if direct.exists() {
            return Ok(direct);
        }
        // yine de bulunan yeri döndür
        return Ok(found);
    }
    Err(anyhow!("eboot.bin bulunamadi: {}", game.display()))
}

/// Update PKG çıkartması içeriği genelde `CUSA03173-patch/CUSA03173-patch/` gibi
/// iç içe kalır. Merge öncesi içerik köküne indir (rename).
pub fn ensure_patch_at_root(patch: &Path) -> Result<PathBuf> {
    ensure_patch_progress(patch, None)
}

pub fn ensure_patch_progress(
    patch: &Path,
    on_progress: Option<&(dyn Fn(u8, String) + Sync)>,
) -> Result<PathBuf> {
    flatten_nested_titles(patch, on_progress);
    let direct = patch.join("eboot.bin");
    if direct.exists() {
        return Ok(direct);
    }
    for name in ["CUSA03173-patch", "CUSA03173"] {
        let nested = patch.join(name);
        if nested.is_dir() {
            if nested.join("eboot.bin").exists() || nested.join("sce_sys").is_dir() {
                eprintln!("[extract] patch flatten: {} -> {}", nested.display(), patch.display());
                flatten_by_move(&nested, patch, on_progress)?;
                let _ = std::fs::remove_dir_all(&nested);
                if patch.join("eboot.bin").exists() {
                    return Ok(patch.join("eboot.bin"));
                }
            }
        }
    }
    if let Some(found) = find_named(patch, "eboot.bin") {
        let parent = found.parent().unwrap_or(patch).to_path_buf();
        if parent != patch {
            eprintln!("[extract] patch flatten: {} -> {}", parent.display(), patch.display());
            flatten_by_move(&parent, patch, on_progress)?;
            if parent.starts_with(patch) {
                let _ = std::fs::remove_dir_all(&parent);
                if patch.join("eboot.bin").exists() {
                    return Ok(patch.join("eboot.bin"));
                }
            }
        }
        return Ok(found);
    }
    Err(anyhow!("update eboot.bin bulunamadi: {}", patch.display()))
}

/// Damga tek başına yeterli değil: game/eboot ile patch eboot boyutu aynıysa
/// update gerçekten uygulanmıştır.
pub fn update_matches_game(game: &Path, patch: &Path) -> bool {
    let game_e = game.join("eboot.bin");
    let patch_e = find_named(patch, "eboot.bin").unwrap_or_else(|| patch.join("eboot.bin"));
    match (std::fs::metadata(&game_e), std::fs::metadata(&patch_e)) {
        (Ok(a), Ok(b)) if a.is_file() && b.is_file() && a.len() == b.len() && a.len() > 0 => true,
        _ => false,
    }
}

/// Yanl?? uygulanm?? update damgas?n? d???r (boyut uyu?muyorsa).
pub fn invalidate_misapplied_update_stamp(game: &Path, patch: &Path) {
    let stamp = game.join(".bb-update-ok");
    if stamp.exists() && !update_matches_game(game, patch) {
        let _ = std::fs::remove_file(&stamp);
        eprintln!("[extract] update damgasi vardi ama eboot ustune binmedi - damga kaldirildi");
    }
}

/// stdout `\r`/`\n` kar???k sat?r basar; her sat?r? (pct, detail) olarak kanala yolla.
struct Pump {
    pending: Vec<u8>,
    last_pct: u8,
    last_emit: u64,
    known_tot: u64,
}

impl Pump {
    fn new() -> Self {
        Self {
            pending: Vec::new(),
            last_pct: 255,
            last_emit: 0,
            known_tot: 0,
        }
    }

    fn feed(
        &mut self,
        chunk: &[u8],
        tx: &mpsc::Sender<(u8, String)>,
        done_seen: &AtomicBool,
    ) {
        self.pending.extend_from_slice(chunk);
        let mut start = 0usize;
        for i in 0..self.pending.len() {
            if self.pending[i] != b'\n' && self.pending[i] != b'\r' {
                continue;
            }
            if i > start {
                let line = String::from_utf8_lossy(&self.pending[start..i]).to_string();
                self.handle_line(&line, tx, done_seen);
            }
            start = i + 1;
        }
        if start > 0 {
            self.pending.drain(..start);
        }
        if let Ok(text) = std::str::from_utf8(&self.pending) {
            let owned = text.to_string();
            self.handle_line(&owned, tx, done_seen);
        }
        if self.pending.len() > 16384 {
            let keep = self.pending.len().saturating_sub(4096);
            self.pending.drain(..keep);
        }
    }

    fn handle_line(&mut self, line: &str, tx: &mpsc::Sender<(u8, String)>, done_seen: &AtomicBool) {
        if let Some((cur, tot)) = parse_extract_line(line) {
            self.known_tot = tot;
            let pct = ((cur as f64 / tot as f64) * 100.0).floor().min(100.0) as u8;
            // %100 ? kill YOK: PKG?de faz sayac? erken dolabilir, dosya yaz?m? s?rer
            if cur >= tot {
                self.last_pct = 100;
                self.last_emit = cur;
                let _ = tx.send((100, "bitti".to_string()));
            } else if pct != self.last_pct || cur.saturating_sub(self.last_emit) >= 100 {
                self.last_pct = pct;
                self.last_emit = cur;
                let _ = tx.send((pct, format!("{cur} / {tot}")));
            }
        }
        let lower = line.to_ascii_lowercase();
        if lower.contains("the end")
            || lower.contains("press [enter]")
            || lower.contains("press enter")
        {
            done_seen.store(true, Ordering::SeqCst);
        }
    }
}

/// Dizin alt?ndaki dosya say?s? + toplam bayt (uygunsuz kill i?in).
pub fn tree_stats(path: &Path) -> (u64, u64) {
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            let Ok(md) = std::fs::symlink_metadata(&p) else { continue };
            if md.file_type().is_symlink() {
                continue;
            }
            if md.is_dir() {
                stack.push(p);
            } else {
                files += 1;
                bytes += md.len();
            }
        }
    }
    (files, bytes)
}

pub fn tree_bytes(path: &Path) -> u64 {
    tree_stats(path).1
}

/// Golden a?a? manifesti: `relative/path\tsize` sat?rlar?.
/// Base: downloads\CUSA03173 ? Update: downloads\CUSA03173-patch
pub const BASE_MANIFEST: &str = include_str!("../expected-base.tsv");
pub const PATCH_MANIFEST: &str = include_str!("../expected-patch.tsv");

fn normalize_rel(rel: &str) -> String {
    rel.replace('\\', "/")
}

/// Manifest?teki her dosya root alt?nda ayn? boyutta olmal?.
/// `eboot_allow` verilirse eboot.bin boyutu o listeden biri olabilir
/// (merge sonras? patch eboot?u ?st?ne binmi? olabilir).
pub fn verify_tree_manifest(
    manifest: &str,
    root: &Path,
    label: &str,
    eboot_allow: Option<&[u64]>,
) -> Result<()> {
    if !root.is_dir() {
        return Err(anyhow!("{label} klas?r yok: {}", root.display()));
    }
    let mut missing = 0u32;
    let mut bad_size = 0u32;
    let mut samples: Vec<String> = Vec::new();
    let mut checked = 0u64;
    let mut expect_bytes = 0u64;
    let mut eboot_ok = false;

    // Manifest bos (Recuva/placeholder) ise dogrulamayi atla; uygulama calissin.
    if manifest.trim().is_empty() {
        eprintln!("[extract] {label} manifest bos, dogrulama atlandi: {}", root.display());
        return Ok(());
    }

    for line in manifest.lines() {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            continue;
        }
        let Some((rel, size_s)) = line.split_once('\t') else {
            continue;
        };
        let rel = normalize_rel(rel);
        let Ok(size) = size_s.trim().parse::<u64>() else {
            continue;
        };
        expect_bytes += size;
        checked += 1;
        let path = root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        match std::fs::metadata(&path) {
            Err(_) => {
                missing += 1;
                if samples.len() < 10 {
                    samples.push(format!("eksik: {rel}"));
                }
            }
            Ok(md) if md.is_file() => {
                let got = md.len();
                let allow = if rel.eq_ignore_ascii_case("eboot.bin") {
                    match eboot_allow {
                        Some(list) => list.contains(&got),
                        None => got == size,
                    }
                } else {
                    got == size
                };
                if rel.eq_ignore_ascii_case("eboot.bin") && allow {
                    eboot_ok = true;
                }
                if !allow {
                    bad_size += 1;
                    if samples.len() < 10 {
                        samples.push(format!("boyut: {rel} {got} != {size}"));
                    }
                }
            }
            Ok(_) => {
                missing += 1;
                if samples.len() < 10 {
                    samples.push(format!("dosya degil: {rel}"));
                }
            }
        }
    }

    if eboot_allow.is_some() && !eboot_ok && missing == 0 && bad_size == 0 {
        // eboot listede vard? ve boyut uymad?ysa zaten bad_size artt?
    }

    // Nested art?k: toplam dosya/bayt manifest ile birebir olmal?
    let (files, bytes) = tree_stats(root);
    if files != checked || bytes != expect_bytes {
        // eboot merge sonras? boyut fark? bayt toplam?n? kayd?r?r
        let eboot_delta = if eboot_allow.is_some() {
            // izin verilen eboot boyutlar? aras?ndaki fark (merge sonras?)
            // manifest eboot boyutu ile ger?ek aras?ndaki fark? bytes?a yans?
            let man_eboot: u64 = manifest
                .lines()
                .find_map(|l| {
                    let (r, s) = l.split_once('\t')?;
                    if normalize_rel(r).eq_ignore_ascii_case("eboot.bin") {
                        s.trim().parse().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            let real_eboot = std::fs::metadata(root.join("eboot.bin"))
                .map(|m| m.len())
                .unwrap_or(0);
            real_eboot.saturating_sub(man_eboot) as i64
        } else {
            0
        };
        let bytes_adj = bytes as i64 - eboot_delta;
        if files != checked || bytes_adj != expect_bytes as i64 {
            return Err(anyhow!(
                "{label} toplam uyusmuyor: {files} dosya / {bytes} bayt - beklenen {checked} / {expect_bytes} (nested artik olabilir)"
            ));
        }
    }

    if missing == 0 && bad_size == 0 {
        Ok(())
    } else {
        Err(anyhow!(
            "{label} dogrulama basarisiz: {missing} eksik, {bad_size} boyut farki / {checked} dosya. Ornek: {}",
            samples.join("; ")
        ))
    }
}

pub fn verify_base_tree(game: &Path) -> Result<()> {
    verify_tree_manifest(
        BASE_MANIFEST,
        game,
        "oyun (downloads\\CUSA03173)",
        Some(&[BASE_EBOOT_SIZE, PATCH_EBOOT_SIZE]),
    )
}

pub fn verify_patch_tree(patch: &Path) -> Result<()> {
    verify_tree_manifest(
        PATCH_MANIFEST,
        patch,
        "update (downloads\\CUSA03173-patch)",
        Some(&[PATCH_EBOOT_SIZE]),
    )
}

/// Flatten sonras? i? i?e kalan title klas?r?n? sil (CUSA03173/CUSA03173/?).
pub fn drop_nested_title_dir(root: &Path, title: &str) {
    let nested = root.join(title);
    if !nested.is_dir() {
        return;
    }
    if !root.join("eboot.bin").exists() {
        return;
    }
    if nested.join("eboot.bin").exists() || nested.join("sce_sys").is_dir() {
        eprintln!("[extract] nested temizleniyor: {}", nested.display());
        let _ = std::fs::remove_dir_all(&nested);
    }
}

pub fn run_pkg_extractor(
    exe: &Path,
    pkg: &Path,
    out: &Path,
    on_progress: &(dyn Fn(u8, String) + Sync),
) -> Result<()> {
    if !exe.exists() {
        return Err(anyhow!("pkg_extractor bulunamadi: {}", exe.display()));
    }
    if !pkg.exists() {
        return Err(anyhow!("pkg bulunamadi: {}", pkg.display()));
    }
    std::fs::create_dir_all(out)?;

    // cwd asla src-tauri olmas?n: extractor oraya bir ?ey yazarsa tauri dev rebuild eder
    let work_dir = exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| out.to_path_buf());
    let mut child = Command::new(exe)
        .arg(pkg)
        .arg(out)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // NUL: cin.get() hemen EOF versin ? yine de baz? build'ler konsol bekliyor
        .stdin(Stdio::null())
        .current_dir(&work_dir)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .spawn()
        .context("pkg_extractor calistirilamadi")?;

    let stdout = child.stdout.take().context("stdout al?namad?")?;
    let stderr = child.stderr.take();

    let done = Arc::new(AtomicBool::new(false));
    let done_seen = Arc::new(AtomicBool::new(false));
    let known_tot = Arc::new(AtomicU64::new(0));
    let last_stdout_at = Arc::new(Mutex::new(Instant::now()));
    let (tx, rx) = mpsc::channel::<(u8, String)>();
    let out_path = out.to_path_buf();

    // Reader'lar sadece kanala yazar ? on_progress'? ?a??rmaz (join deadlock yok)
    let stdout_thread = {
        let tx = tx.clone();
        let done_seen = done_seen.clone();
        let known_tot = known_tot.clone();
        let last_stdout_at = last_stdout_at.clone();
        std::thread::spawn(move || {
            let mut stdout = stdout;
            let mut pump = Pump::new();
            let mut buf = [0u8; 8192];
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Ok(mut ts) = last_stdout_at.lock() {
                            *ts = Instant::now();
                        }
                        pump.feed(&buf[..n], &tx, &done_seen);
                        known_tot.store(pump.known_tot, Ordering::SeqCst);
                    }
                    Err(_) => break,
                }
            }
            if !pump.pending.is_empty() {
                let pending = std::mem::take(&mut pump.pending);
                pump.feed(&pending, &tx, &done_seen);
            }
        })
    };

    let stderr_thread = {
        let tx = tx.clone();
        let done_seen = done_seen.clone();
        let known_tot = known_tot.clone();
        let last_stdout_at = last_stdout_at.clone();
        std::thread::spawn(move || {
            let mut stderr = match stderr {
                Some(s) => s,
                None => return Vec::new(),
            };
            let mut pump = Pump::new();
            let mut all: Vec<u8> = Vec::new();
            let mut tmp = [0u8; 4096];
            loop {
                match stderr.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Ok(mut ts) = last_stdout_at.lock() {
                            *ts = Instant::now();
                        }
                        pump.feed(&tmp[..n], &tx, &done_seen);
                        known_tot.store(pump.known_tot, Ordering::SeqCst);
                        all.extend_from_slice(&tmp[..n]);
                    }
                    Err(_) => break,
                }
            }
            all
        })
    };

    // Disk ilerlemesi: dosya sayisi + bayt. Durgunluk = bayt sabit, kill icin cok daha uzun.
    let last_fs_change = Arc::new(Mutex::new(Instant::now()));
    let last_bytes = Arc::new(AtomicU64::new(0));
    let fs_thread = {
        let tx = tx.clone();
        let done = done.clone();
        let known_tot = known_tot.clone();
        let last_stdout_at = last_stdout_at.clone();
        let last_fs_change = last_fs_change.clone();
        let last_bytes = last_bytes.clone();
        std::thread::spawn(move || {
            let mut last_pct = 255u8;
            let mut last_emit = 0u64;
            let mut last_files = 0u64;
            let mut last_bytes_val = 0u64;
            while !done.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_secs(2));
                let silent = last_stdout_at
                    .lock()
                    .map(|t| t.elapsed() >= Duration::from_millis(1500))
                    .unwrap_or(true);
                if !silent {
                    continue;
                }
                let (cur, bytes) = tree_stats(&out_path);
                if cur != last_files || bytes != last_bytes_val {
                    last_files = cur;
                    last_bytes_val = bytes;
                    last_bytes.store(bytes, Ordering::SeqCst);
                    if let Ok(mut ts) = last_fs_change.lock() {
                        *ts = Instant::now();
                    }
                }
                if cur == 0 {
                    continue;
                }
                let tot = known_tot.load(Ordering::SeqCst);
                if tot > 0 {
                    // sadece UI ? dosya sayisi tot'a esit diye is bitmis sayilma
                    if cur >= tot {
                        last_pct = 100;
                        last_emit = cur;
                        let _ = tx.send((100u8, "bitti".to_string()));
                    } else {
                        let pct = ((cur as f64 / tot as f64) * 100.0).floor().min(99.0) as u8;
                        if pct != last_pct || cur.saturating_sub(last_emit) >= 100 {
                            last_pct = pct;
                            last_emit = cur;
                            let _ = tx.send((last_pct, format!("{cur} / {tot}")));
                        }
                    }
                } else if cur.saturating_sub(last_emit) >= 25 {
                    last_emit = cur;
                    if last_pct == 255 {
                        last_pct = 1;
                    }
                    let _ = tx.send((last_pct, format!("{cur} dosya")));
                }
            }
        })
    };

    drop(tx); // main disinda kalan sender'lar reader'larda

    // Ana dongu: kanal + try_wait. Kill sadece gercek durgunluk veya "the end" + kapanmayan surec.
    let mut exit_status: Option<Result<std::process::ExitStatus>> = None;
    let mut done_at: Option<Instant> = None;
    let mut last_pct_emitted = 255u8;
    let start = Instant::now();

    loop {
        while let Ok((pct, detail)) = rx.try_recv() {
            if pct > last_pct_emitted {
                last_pct_emitted = pct;
            }
            // pct=100 artik done_seen tetiklemiyor; "bitti" UI'da %100 g?rs?n
            let out_pct = if detail == "bitti" { 100 } else { pct.min(99) };
            on_progress(out_pct, detail);
        }

        match child.try_wait() {
            Ok(Some(st)) => {
                exit_status = Some(Ok(st));
                eprintln!("[extract] extractor kendi kapandi: {:?}", st.code());
                break;
            }
            Ok(None) => {}
            Err(e) => {
                exit_status = Some(Err(anyhow!(e).context("pkg_extractor beklenemedi")));
                break;
            }
        }

        let fs_idle = last_fs_change
            .lock()
            .map(|t| t.elapsed() >= Duration::from_secs(90))
            .unwrap_or(true);
        let stdout_silent = last_stdout_at
            .lock()
            .map(|t| t.elapsed() >= Duration::from_secs(20))
            .unwrap_or(true);

        // true hang: stdout yok + bayt 90sn degismemis
        if stdout_silent && fs_idle && start.elapsed() >= Duration::from_secs(60) {
            eprintln!("[extract] stdout+disk 90sn durgun ? kill");
            let _ = child.kill();
            let mut waited = None;
            for _ in 0..40 {
                match child.try_wait() {
                    Ok(Some(st)) => {
                        waited = Some(Ok(st));
                        break;
                    }
                    Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                    Err(e) => {
                        waited = Some(Err(anyhow!(e)));
                        break;
                    }
                }
            }
            exit_status = waited;
            break;
        }

        // "the end"/"press enter" ama surec hala yaziyorsa bekle
        if done_seen.load(Ordering::SeqCst) {
            if done_at.is_none() {
                done_at = Some(Instant::now());
                eprintln!("[extract] bitti sinyali ? surec kapanana kadar bekleme");
            }
            if let Some(t) = done_at {
                let bytes_idle = last_fs_change
                    .lock()
                    .map(|x| x.elapsed() >= Duration::from_secs(20))
                    .unwrap_or(true);
                // 60sn kapanmadi VE yazim durduysa kill (cin.get tuza??)
                if t.elapsed() >= Duration::from_secs(60) && bytes_idle {
                    eprintln!("[extract] the end sonras? yaz?m durdu, kapanmad? ? kill");
                    let _ = child.kill();
                    let mut waited = None;
                    for _ in 0..40 {
                        match child.try_wait() {
                            Ok(Some(st)) => {
                                waited = Some(Ok(st));
                                break;
                            }
                            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                            Err(e) => {
                                waited = Some(Err(anyhow!(e)));
                                break;
                            }
                        }
                    }
                    exit_status = waited;
                    break;
                }
            }
        }

        // 30GB cikartma HDD'de 2 saati gecebilir ? kill siniri uzun
        if start.elapsed() > Duration::from_secs(120 * 60) {
            eprintln!("[extract] 120 dk siniri ? kill");
            let _ = child.kill();
            let _ = child.try_wait();
            break;
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    done.store(true, Ordering::SeqCst);
    while let Ok((pct, detail)) = rx.try_recv() {
        if pct > last_pct_emitted {
            last_pct_emitted = pct;
        }
        on_progress(pct.min(99), detail);
    }

    let _ = stdout_thread;
    let err_bytes = {
        let (etx, erx) = mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let bytes = stderr_thread.join().unwrap_or_default();
            let _ = etx.send(bytes);
        });
        erx.recv_timeout(Duration::from_secs(2)).unwrap_or_default()
    };
    let _ = {
        let (etx, erx) = mpsc::channel::<()>();
        std::thread::spawn(move || {
            let _ = fs_thread.join();
            let _ = etx.send(());
        });
        erx.recv_timeout(Duration::from_secs(2))
    };

    let killed_for_stall = exit_status.is_none();
    let files_out = count_files_tree(out);
    let bytes_out = tree_bytes(out);
    eprintln!("[extract] cikartma ozeti: {files_out} dosya, {bytes_out} bayt");
    extract_log(&format!(
        "[extract] pkg_extractor: files={files_out} bytes={bytes_out}"
    ));

    match exit_status {
        Some(Ok(st)) => {
            if !st.success() && files_out == 0 {
                let err_text = String::from_utf8_lossy(&err_bytes);
                let tail = err_text
                    .lines()
                    .rev()
                    .take(8)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n");
                return Err(anyhow!(
                    "pkg_extractor hata kodu {}{}",
                    st.code().unwrap_or(-1),
                    if tail.trim().is_empty() {
                        String::new()
                    } else {
                        format!(" ? {}", tail.trim())
                    }
                ));
            }
            if !st.success() {
                eprintln!(
                    "[extract] extractor exit {:?} ama dosya var ? devam",
                    st.code()
                );
            }
        }
        Some(Err(e)) => {
            if files_out == 0 {
                return Err(e);
            }
            eprintln!("[extract] {e} ? dosya var, devam");
        }
        None => {
            if files_out == 0 {
                return Err(anyhow!("pkg_extractor durumu bilinmiyor"));
            }
            eprintln!("[extract] surec birakildi ? dosya var, devam");
        }
    }

    if last_pct_emitted < 100 {
        on_progress(100, String::new());
    }
    let _ = killed_for_stall;
    Ok(())
}

pub fn extract_zip(zip_path: &Path, dest: &Path) -> Result<()> {
    extract_zip_with_progress(zip_path, dest, None)
}

/// Dosya dosya ??kart?r; on_progress (y?zde, "cur / tot") ile ilerler.
pub fn extract_zip_with_progress(
    zip_path: &Path,
    dest: &Path,
    on_progress: Option<&(dyn Fn(u8, String) + Sync)>,
) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    let file = std::fs::File::open(zip_path).context("zip acilamadi")?;
    let mut archive = zip::ZipArchive::new(file).context("zip okunamadi")?;
    let total = archive.len();
    if total == 0 {
        return Ok(());
    }
    let mut last_pct = 255u8;
    for i in 0..total {
        let mut entry = archive.by_index(i).context("zip uye okunamadi")?;
        let rel = match entry.enclosed_name() {
            Some(path) => path,
            None => continue,
        };
        let outpath = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&outpath).context("zip ??kart?lamad?")?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).context("zip ??kart?lamad?")?;
            }
            let mut outfile = std::fs::File::create(&outpath).context("zip ??kart?lamad?")?;
            std::io::copy(&mut entry, &mut outfile).context("zip ??kart?lamad?")?;
            use std::io::Write as _;
            let _ = outfile.flush();
        }
        if let Some(cb) = on_progress {
            let cur = i + 1;
            let pct = ((cur as f64 / total as f64) * 100.0).floor().min(100.0) as u8;
            if pct != last_pct {
                last_pct = pct;
                cb(pct, format!("{cur} / {total}"));
            }
        }
    }
    if let Some(cb) = on_progress {
        cb(100, format!("{total} / {total}"));
    }
    eprintln!("[extract] zip bitti ({total} ?ye) ? normalize/ta??ma ba?l?yor");
    Ok(())
}

/// Port ar?ivindeki i? i?e klas?r? d?zle?tirir; ilerleme (y?zde, "cur / tot").
pub fn normalize_port_zip_with_progress(
    dest: &Path,
    on_progress: Option<&(dyn Fn(u8, String) + Sync)>,
) -> Result<()> {
    let nested = dest.join("bbport-windows");
    if nested.is_dir() {
        // tmp ta??nd?ysa nested'? da yan?na al ? as?l normalize nested ? dest
        let total = count_files(&nested).max(1);
        extract_log(&format!("[extract] normalize: bbport-windows ? dest ({total} dosya)"));
        if let Some(cb) = on_progress {
            cb(0, format!("0 / {total}"));
        }
        let mut done = 0u64;
        let mut last_pct = 255u8;
        merge_recursive(&nested, dest, &mut done, total, &mut last_pct, on_progress)?;
        // nested silinmezken dest'e kopyalanm?? dosyalar kal?r; nested'? temizle
        extract_log("[extract] normalize: nested siliniyor");
        let _ = std::fs::remove_dir_all(&nested);
        extract_log("[extract] normalize: nested silindi");
        if let Some(cb) = on_progress {
            cb(100, format!("{total} / {total}"));
        }
    }

    // nested bitti ? UI asla "420/420"da kalmas?n; exe faz?na ge?
    if let Some(cb) = on_progress {
        cb(100, "exe aranıyor".into());
    }
    extract_log(&format!("[extract] normalize: exe kontrolu {}", dest.display()));

    let direct = dest.join("Bloodborne.exe");
    if direct.exists() {
        extract_log("[extract] normalize: Bloodborne.exe k?kte");
        return Ok(());
    }
    let candidates = [
        dest.join("Bloodborne Launcher.exe"),
        dest.join("Play Bloodborne.exe"),
    ];
    if let Some(hit) = candidates.iter().find(|p| p.exists()) {
        extract_log(&format!("[extract] normalize: launcher exe {}", hit.display()));
        return Ok(());
    }
    if let Some(found) = find_named(dest, "Bloodborne.exe") {
        let parent = found.parent().unwrap_or(dest).to_path_buf();
        if parent != dest {
            extract_log(&format!("[extract] normalize: flatten {}", parent.display()));
            if let Some(cb) = on_progress {
                cb(0, "0".into());
            }
            merge_dir(&parent, dest)?;
            if let Some(cb) = on_progress {
                cb(100, "100".into());
            }
        }
    } else {
        extract_log("[extract] normalize: Bloodborne.exe bulunamadi");
    }
    Ok(())
}

pub fn merge_dir(from: &Path, to: &Path) -> Result<()> {
    merge_dir_with_progress(from, to, None)
}

/// Dosya say?s?n? sayarak birle?tirir; on_progress (y?zde, "cur / tot").
pub fn merge_dir_with_progress(
    from: &Path,
    to: &Path,
    on_progress: Option<&(dyn Fn(u8, String) + Sync)>,
) -> Result<()> {
    let total = count_files(from).max(1);
    let mut done = 0u64;
    let mut last_pct = 255u8;
    merge_recursive(from, to, &mut done, total, &mut last_pct, on_progress)?;
    if let Some(cb) = on_progress {
        cb(100, format!("{total} / {total}"));
    }
    Ok(())
}

fn count_files(path: &Path) -> u64 {
    let mut n = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                n += count_files(&p);
            } else {
                n += 1;
            }
        }
    }
    n
}

/// Konsol borusu dolarsa eprintln as?l? kal?r; kritik sat?rlar? dosyaya da yaz.
///
/// Log ASLA proje a?ac?nda olmamal?: `tauri dev` `src-tauri\` i?indeki her
/// yaz?m? yakalay?p rebuild/restart yapar; ??kartma ortada kesilir,
/// `.bb-base-ok` yaz?lmaz, sonraki a??l?? `game/`'i silip ba?tan ??kar?r.
fn extract_log_path() -> PathBuf {
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        let dir = PathBuf::from(base).join("BloodborneInstaller");
        if std::fs::create_dir_all(&dir).is_ok() {
            return dir.join("extract.log");
        }
    }
    if let Some(base) = std::env::var_os("TEMP") {
        let dir = PathBuf::from(base).join("BloodborneInstaller");
        if std::fs::create_dir_all(&dir).is_ok() {
            return dir.join("extract.log");
        }
    }
    std::env::temp_dir().join("bloodborne-extract.log")
}

pub fn extract_log(msg: &str) {
    eprintln!("{msg}");
    let path = extract_log_path();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write as _;
        let _ = writeln!(f, "{msg}");
    }
}

fn merge_recursive(
    from: &Path,
    to: &Path,
    done: &mut u64,
    total: u64,
    last_pct: &mut u8,
    on_progress: Option<&(dyn Fn(u8, String) + Sync)>,
) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            merge_recursive(&src, &dst, done, total, last_pct, on_progress)?;
        } else {
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&src, &dst).with_context(|| {
                format!("kopyalanamadi {} -> {}", src.display(), dst.display())
            })?;
            *done += 1;
            if let Some(cb) = on_progress {
                let pct = ((*done as f64 / total as f64) * 100.0).floor().min(100.0) as u8;
                if pct != *last_pct {
                    *last_pct = pct;
                    cb(pct, format!("{done} / {total}"));
                }
            }
        }
    }
    Ok(())
}

/// bbport oyun + launcher ayarlar? (settings.json + bbport.ini).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PortSettings {
    pub game_dir: String,
    pub user_dir: String,
    pub fps_mode: String,
    pub upscaler: String,
    pub preset: String,
    pub output_res: String,
}

/// Varsay?lan kay?t klas?r?: C:\Users\<user>\Documents\Bloodborne\saves
pub fn default_user_dir() -> PathBuf {
    dirs::document_dir()
        .or_else(|| {
            std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Documents"))
        })
        .unwrap_or_else(|| PathBuf::from(r"C:\Users\Public\Documents"))
        .join("Bloodborne")
        .join("saves")
}

fn launcher_settings_file() -> Option<PathBuf> {
    let appdata =
        dirs::data_dir().or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))?;
    Some(appdata.join("bbport-launcher").join("settings.json"))
}

fn port_ini_path(root: &Path) -> PathBuf {
    crate::state::port_dir(root).join("bbport.ini")
}

fn read_ini_map(path: &Path) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let Ok(text) = std::fs::read_to_string(path) else {
        return map;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

fn write_ini_updates(path: &Path, updates: &[(&str, &str)]) -> Result<()> {
    let mut lines: Vec<String> = if path.exists() {
        std::fs::read_to_string(path)?
            .lines()
            .map(|s| s.to_string())
            .collect()
    } else {
        vec!["# bbport settings (in-game menu: Insert / L3+R3)".into()]
    };
    let mut written = std::collections::HashSet::new();
    for line in lines.iter_mut() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || !trimmed.contains('=') {
            continue;
        }
        let key = trimmed
            .split_once('=')
            .map(|(k, _)| k.trim().to_string())
            .unwrap_or_default();
        if key.is_empty() {
            continue;
        }
        if let Some((_, value)) = updates.iter().find(|(k, _)| *k == key) {
            *line = format!("{key}={value}");
            written.insert(key);
        }
    }
    for (key, value) in updates {
        if !written.contains(*key) {
            lines.push(format!("{key}={value}"));
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

/// Kurulum k?k?nden + varsa mevcut dosyalardan bbport ayarlar?n? oku.
/// Eksik anahtarlar i?in varsay?lanlar: 90 FPS, FSR4, Quality, 1920x1080.
pub fn read_port_settings(root: &Path) -> PortSettings {
    let game = crate::state::game_dir(root);
    let mut settings = PortSettings {
        game_dir: game.to_string_lossy().replace('/', "\\"),
        user_dir: default_user_dir().to_string_lossy().replace('/', "\\"),
        fps_mode: "90".into(),
        upscaler: "fsr4".into(),
        preset: "1".into(),
        output_res: "1920x1080".into(),
    };
    if let Some(path) = launcher_settings_file() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(obj) = value.as_object() {
                    if let Some(s) = obj.get("game_dir").and_then(|v| v.as_str()) {
                        if !s.is_empty() {
                            settings.game_dir = s.to_string();
                        }
                    }
                    if let Some(s) = obj.get("user_dir").and_then(|v| v.as_str()) {
                        if !s.is_empty() {
                            settings.user_dir = s.to_string();
                        }
                    }
                    if let Some(s) = obj.get("fps_mode").and_then(|v| v.as_str()) {
                        if !s.is_empty() {
                            settings.fps_mode = s.to_string();
                        }
                    }
                }
            }
        }
    }
    let ini = read_ini_map(&port_ini_path(root));
    if let Some(s) = ini.get("upscaler").filter(|s| !s.is_empty()) {
        settings.upscaler = s.clone();
    }
    if let Some(s) = ini.get("preset").filter(|s| !s.is_empty()) {
        settings.preset = s.clone();
    }
    if let Some(s) = ini.get("output_res").filter(|s| !s.is_empty()) {
        settings.output_res = s.clone();
    }
    settings
}

/// Ayarlar? hem settings.json hem bbport.ini i?ine yaz; kay?t klas?r?n? olu?tur.
pub fn write_port_settings(root: &Path, settings: &PortSettings) -> Result<()> {
    let user = PathBuf::from(settings.user_dir.trim());
    std::fs::create_dir_all(&user).ok();

    if let Some(path) = launcher_settings_file() {
        let mut value = if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            serde_json::from_str(&text).unwrap_or_else(|_| default_settings())
        } else {
            default_settings()
        };
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "game_dir".into(),
                serde_json::Value::String(settings.game_dir.replace('/', "\\")),
            );
            obj.insert(
                "user_dir".into(),
                serde_json::Value::String(settings.user_dir.replace('/', "\\")),
            );
            obj.insert(
                "fps_mode".into(),
                serde_json::Value::String(settings.fps_mode.clone()),
            );
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(&value)?)?;
    }

    write_ini_updates(
        &port_ini_path(root),
        &[
            ("upscaler", settings.upscaler.as_str()),
            ("preset", settings.preset.as_str()),
            ("output_res", settings.output_res.as_str()),
        ],
    )?;
    Ok(())
}

/// Kurulum bitince varsay?lanlar? bas: oyun klas?r?, Documents\Bloodborne\saves,
/// 90 FPS, FSR4, Quality (preset=1), 1920x1080.
pub fn write_launcher_settings(game_path: &Path) -> Result<()> {
    let root = game_path
        .parent()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| game_path.to_path_buf());
    let settings = PortSettings {
        game_dir: game_path.to_string_lossy().replace('/', "\\"),
        user_dir: default_user_dir().to_string_lossy().replace('/', "\\"),
        fps_mode: "90".into(),
        upscaler: "fsr4".into(),
        preset: "1".into(),
        output_res: "1920x1080".into(),
    };
    write_port_settings(&root, &settings)
}

fn default_settings() -> serde_json::Value {
    serde_json::json!({
        "ui_language": "",
        "game_dir": "",
        "user_dir": "",
        "dlc_dir": "",
        "mods_dir": "",
        "mods_enabled": true,
        "patches_dir": "",
        "language": "1",
        "voice_language": "auto",
        "player_name": "",
        "fullscreen": true,
        "hdr": false,
        "present_mode": "Mailbox",
        "fps_mode": "90",
        "frame_cap": "120",
        "draw_pipe": "0",
        "readbacks": "",
        "frames_ahead": "",
        "frame_stats": false,
        "gpu_profile": false,
        "vk_validation": false,
        "extra_env": "",
        "close_on_play": false,
        "check_updates": true,
        "online": false,
        "online_server": "srv.shadps4.net:31313",
        "online_webapi": "",
        "online_npid": "",
        "online_password": "",
        "online_upnp": true,
        "online_community": "https://thehuntersdream.com",
        "online_pool": "shadnet",
        "bridge_live_map": true,
        "supermedo_user": "",
        "supermedo_password": ""
    })
}

/// Verilen klasörde (port kökü veya kurulum kökü) oyun/launcher exe'sini bul.
pub fn port_exe(path: &Path) -> PathBuf {
    fn exe_in(dir: &Path) -> Option<PathBuf> {
        let direct = dir.join("Bloodborne.exe");
        if direct.exists() {
            return Some(direct);
        }
        for name in ["Bloodborne Launcher.exe", "Play Bloodborne.exe"] {
            let p = dir.join(name);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    if let Some(p) = exe_in(path) {
        return p;
    }
    let nested = path.join("port");
    if nested.is_dir() {
        if let Some(p) = exe_in(&nested) {
            return p;
        }
        if let Some(found) = find_named(&nested, "Bloodborne.exe") {
            return found;
        }
    }
    if let Some(found) = find_named(path, "Bloodborne.exe") {
        return found;
    }
    if path.file_name().map(|n| n == "port").unwrap_or(false) {
        path.join("Bloodborne.exe")
    } else {
        nested.join("Bloodborne.exe")
    }
}

fn find_named_exe(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    for name in names {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    let nested = dir.join("port");
    if nested.is_dir() {
        for name in names {
            let p = nested.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
        for name in names {
            if let Some(found) = find_named(&nested, name) {
                return Some(found);
            }
        }
    }
    for name in names {
        if let Some(found) = find_named(dir, name) {
            return Some(found);
        }
    }
    None
}

/// Oyna butonu: Play Bloodborne.exe (yoksa launcher, yoksa oyun).
pub fn play_exe(path: &Path) -> PathBuf {
    find_named_exe(
        path,
        &["Play Bloodborne.exe", "Bloodborne Launcher.exe", "Bloodborne.exe"],
    )
    .unwrap_or_else(|| port_exe(path))
}

/// Gelişmiş/oyun motoru: Bloodborne.exe
pub fn game_exe(path: &Path) -> PathBuf {
    find_named_exe(path, &["Bloodborne.exe"]).unwrap_or_else(|| port_exe(path))
}

pub fn extractor_zip_path(downloads: &Path) -> PathBuf {
    downloads.join(EXTRACTOR_ZIP_NAME)
}

#[cfg(test)]
mod verify_tests {
    use super::*;

    #[test]
    fn golden_trees_match_manifests() {
        // ASCII-safe: ? (U+0130) indirilenler klas?r ad?nda
        let indirilenler = format!("D:\\{}ndirilenler", "\u{130}");
        let root = Path::new(&indirilenler)
            .join("Bloodborne")
            .join("Installer")
            .join("downloads");
        let game = root.join("CUSA03173");
        let patch = root.join("CUSA03173-patch");
        if game.is_dir() {
            verify_base_tree(&game).expect("base golden tree");
        }
        if patch.is_dir() {
            verify_patch_tree(&patch).expect("patch golden tree");
        }
    }
}
