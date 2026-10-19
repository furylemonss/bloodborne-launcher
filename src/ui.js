import { icons } from "./icons.js";
import { getLang, t, tBadge, tFile, tStatus, tStep } from "./i18n.js";
import {
  formatBytes,
  formatEta,
  formatSpeed,
  pulseReady,
  setBar,
  stagger,
} from "./anim.js";

const REQUIRED = 65 * 1024 * 1024 * 1024;

export const GALLERY_IMAGES = [
  "https://cdn.wccftech.com/wp-content/uploads/2015/03/Bloodborne2.png",
  "https://images.hdqwalls.com/wallpapers/bloodborne-game-4k-tm.jpg",
  "https://4kwallpapers.com/images/wallpapers/bloodborne-video-game-3840x2160-13077.jpg",
  "https://images.alphacoders.com/534/534920.jpg",
  "https://cdn.mos.cms.futurecdn.net/sfiNueEkXAA7LGkUatQstb.jpg",
  "https://i.redd.it/mebfq8xqeuia1.jpg",
  "https://preview.redd.it/i-want-a-cool-bloodborne-wallpaper-v0-1n54yududgbh1.jpg?width=1080&crop=smart&auto=webp&s=81e7094fcd1bfce3a1dda59b2c8ff704496b6952",
  "https://www.dsogaming.com/wp-content/uploads/2016/09/andreas-stavaas-highresscreenshot00012.jpg",
];

const STEP_ICONS = {
  download_base: icons.download,
  download_update: icons.download,
  download_port: icons.download,
  download_extractor: icons.download,
  extract_base: icons.folder,
  extract_update: icons.folder,
  merge_update: icons.refresh,
  extract_port: icons.folder,
  write_settings: icons.gear,
};

let galleryTimer = null;

/** last_error → { title, body, tech, kind } — kurulum akışında görünecek açıklama */
export function explainError(raw) {
  const tech = raw ? String(raw).trim() : "";
  if (!tech) {
    return { title: t("errGenericTitle"), body: t("errGenericBody"), tech: "", kind: "generic" };
  }
  if (/yetersiz disk|disk kapasitesi|insufficient disk|65 GB|bos:|ENOSPC|ERROR_DISK_FULL|disk doldu|part dosyasına yazılamadı|kopyalanamadi/i.test(tech)) {
    const free = /bos:\s*([\d.,]+)\s*GB/i.exec(tech)?.[1];
    return {
      title: t("errDiskTitle"),
      body: free ? `${t("errDiskBody")} (${t("freeSpace")}: ${free} GB)` : t("errDiskBody"),
      tech,
      kind: "disk",
    };
  }
  if (/eboot/i.test(tech)) {
    return { title: t("errEbootTitle"), body: t("errEbootBody"), tech, kind: "eboot" };
  }
  if (/pkg_extractor|extractor calistir|extractor bulun|PkgExtractor/i.test(tech)) {
    return { title: t("errExtractorTitle"), body: t("errExtractorBody"), tech, kind: "extractor" };
  }
  if (/zip acilamadi|zip okunamadi|zip çıkart|arşiv|archive/i.test(tech)) {
    return { title: t("errZipTitle"), body: t("errZipBody"), tech, kind: "zip" };
  }
  if (/çıkart|cikart|extract|hata kodu/i.test(tech)) {
    return { title: t("errExtractTitle"), body: t("errExtractBody"), tech, kind: "extract" };
  }
  if (
    /indir|download|bağlant|connection|timeout|timed out|reqwest|dns|http|resolve|network|kaldırılamadı|broken pipe|HTTP \d|sunucu yanıt/i.test(
      tech
    )
  ) {
    return { title: t("errDownloadTitle"), body: t("errDownloadBody"), tech, kind: "download" };
  }
  if (/iptal edildi|donduruldu/i.test(tech)) {
    return { title: t("st_paused"), body: tech, tech: "", kind: "paused" };
  }
  return { title: t("errGenericTitle"), body: t("errGenericBody"), tech, kind: "generic" };
}

function errorBannerHtml(state) {
  if (!state || state.status !== "error" || !state.last_error) return "";
  const info = explainError(state.last_error);
  const failed =
    state.steps?.find((s) => s.status === "error") ||
    state.steps?.[state.current_step];
  const failedLabel = failed ? tStep(failed.id) : "";
  const icon =
    info.kind === "disk" ? icons.disk : info.kind === "download" ? icons.download : icons.alert;
  return `
    <div class="error-banner" data-el="error-banner" data-raw="${String(state.last_error).replace(/"/g, "&quot;")}">
      <div class="error-banner-head">
        <span class="error-banner-icon">${icon}</span>
        <div>
          <div class="error-banner-title" data-el="error-title">${info.title}</div>
          ${failedLabel ? `<div class="error-banner-step">${t("errStepFailed")}: ${failedLabel}</div>` : ""}
        </div>
      </div>
      <p class="error-banner-body" data-el="error-body">${info.body}</p>
      ${
        info.tech
          ? `<div class="error-banner-tech" data-el="error-tech"><span>${t("errTechLabel")}</span><code>${info.tech}</code></div>`
          : ""
      }
      <div class="error-banner-actions">
        <button class="btn btn-secondary" data-action="resume">${icons.play} ${t("resume")}</button>
      </div>
    </div>
  `;
}

function refreshErrorBanner(root, state) {
  const existing = root.querySelector('[data-el="error-banner"]');
  if (state?.status === "error" && state.last_error) {
    const html = errorBannerHtml(state);
    if (!existing) {
      const soft = root.querySelector(".soft-content");
      const galleryTop = root.querySelector(".gallery-top");
      if (soft) soft.insertAdjacentHTML("afterbegin", html);
      else if (galleryTop) galleryTop.insertAdjacentHTML("beforebegin", html);
      else root.insertAdjacentHTML("afterbegin", html);
    } else if (existing.getAttribute("data-raw") !== String(state.last_error)) {
      existing.outerHTML = html;
    }
  } else if (existing) {
    existing.remove();
  }
}

export function titlebar() {
  const other = getLang() === "tr" ? "EN" : "TR";
  return `
    <header class="titlebar">
      <div class="brand">
        <img class="brand-logo" src="/logo.png" alt="" draggable="false" />
        <span class="brand-text">Bloodborne Launcher</span>
      </div>
      <div class="titlebar-actions">
        <button class="icon-btn lang-btn" data-action="toggle-lang" title="Language">${other}</button>
        <button class="icon-btn" data-action="minimize" title="${t("minimize")}">${icons.minus}</button>
        <button class="icon-btn" data-action="tray" title="${t("trayHint")}">${icons.tray}</button>
        <button class="icon-btn danger" data-action="quit" title="${t("quit")}">${icons.close}</button>
      </div>
    </header>
  `;
}

export function screenPath(pendingRoot) {
  const pathText = pendingRoot || t("folderNotChosen");
  const activeClass = pendingRoot ? "active" : "";
  return `
    <section class="screen" data-screen="path">
      <img class="hero-logo" src="/logo.png" alt="" draggable="false" />
      <div class="title-block">
        <div class="eyebrow">${t("install")}</div>
        <h1 class="title display">${t("prepTitle")}</h1>
        <p class="subtitle" style="margin-top:14px">${t("prepSub")}</p>
      </div>

      <div class="card pad path-card" style="display:grid;gap:14px">
        <div class="row">
          <div class="grow">
            <div class="eyebrow" style="margin-bottom:8px">${t("rootFolder")}</div>
            <div class="path-display ${activeClass}" data-el="path-label">
              ${icons.folder}
              <span>${pathText}</span>
            </div>
          </div>
          <button class="btn btn-secondary" data-action="browse">${icons.folder} ${t("change")}</button>
        </div>
        <div class="dock">
          <button class="btn btn-primary" data-action="confirm-root" ${pendingRoot ? "" : "disabled"}>
            ${icons.bolt} ${t("startInstall")}
          </button>
          <span class="pill" data-el="disk-pill">${icons.disk} ${t("diskWaiting")}</span>
        </div>
      </div>

      <div class="card gauge-card">
        <div class="gauge" data-el="gauge">
          <svg viewBox="0 0 120 120">
            <defs>
              <linearGradient id="gaugeGrad" x1="0" y1="0" x2="1" y2="1">
                <stop offset="0%" stop-color="#ff3b1f"/>
                <stop offset="100%" stop-color="#ff7a2f"/>
              </linearGradient>
            </defs>
            <circle class="track" cx="60" cy="60" r="54"></circle>
            <circle class="value" cx="60" cy="60" r="54"></circle>
          </svg>
          <div class="gauge-label">
            <div>
              <span data-el="gauge-pct">--</span>
              <small>${t("freeLabel")}</small>
            </div>
          </div>
        </div>
        <div style="display:grid;gap:14px">
          <div class="stat-grid">
            <div class="stat">
              <div class="k">${t("freeSpace")}</div>
              <div class="v" data-el="free-size">--</div>
            </div>
            <div class="stat">
              <div class="k">${t("required")}</div>
              <div class="v" data-el="required-size">--</div>
            </div>
            <div class="stat">
              <div class="k">${t("status")}</div>
              <div class="v" data-el="disk-status">--</div>
            </div>
          </div>
          <p class="subtitle">${t("contentsNote")}</p>
        </div>
      </div>
    </section>
  `;
}

export function screenGallery(state, download) {
  const slides = GALLERY_IMAGES.map(
    (url, i) =>
      `<div class="gallery-slide${i === 0 ? " active" : ""}" style="background-image:url('${url}')"></div>`
  ).join("");

  const status = tStatus(state?.status);
  const hasDl = download && (download.total || 0) > 0;
  const ex = extractInfo(state);
  const pct = hasDl && download.total
    ? Math.min(100, Math.floor((download.downloaded / download.total) * 100))
    : ex
      ? ex.pct
      : overallPercent(state);
  const isError = state?.status === "error" && !!state?.last_error;
  const errInfo = isError ? explainError(state.last_error) : null;
  const titleText = isError ? errInfo.title : status;
  const fileLabel = hasDl ? tFile(download.step_id) : isError ? errInfo.body : currentStepTitle(state);
  const stepLine = isError ? errInfo.body : currentStepTitle(state);
  const exMeta = extractMeta(ex);
  const indeterminate = !hasDl && !ex && state?.status === "extracting";

  return `
    <section class="screen gallery" data-screen="gallery">
      <div class="gallery-bg">${slides}</div>
      <div class="gallery-veil"></div>
      <div class="gallery-dots" data-el="gallery-dots">
        ${GALLERY_IMAGES.map((_, i) => `<span class="dot${i === 0 ? " active" : ""}"></span>`).join("")}
      </div>

      ${isError ? errorBannerHtml(state) : ""}

      <div class="gallery-top${isError ? " has-error" : ""}">
        <div class="eyebrow">${isError ? t("errTitle") : t("flow")}</div>
        <h1 class="title display gallery-title${isError ? " error-title" : ""}" data-el="gallery-title">${titleText}</h1>
        <p class="subtitle gallery-sub" data-el="gallery-sub">${stepLine}</p>
      </div>

      <div class="gallery-bottom">
        <div class="gallery-slim" data-el="gallery-slim">
          <div class="progress-wrap">
            <div class="progress-fill ${indeterminate ? "indeterminate" : ""}" data-el="bar"
              style="width:${pct}%"></div>
          </div>
          <div class="gallery-meta-line">
            <span data-el="file-name">${fileLabel}</span>
            <span class="gallery-meta-right">
              <span data-el="file-bytes">${
                hasDl ? `${formatBytes(download.downloaded)} / ${formatBytes(download.total)}` : exMeta.bytes
              }</span>
              <span data-el="file-speed">${hasDl ? formatSpeed(download.speed_bps) : exMeta.speed}</span>
              <span data-el="file-eta">${hasDl ? formatEta(download.eta_secs) : exMeta.eta}</span>
            </span>
          </div>
          <div class="gallery-minimal-actions">
            <button class="mini-link" data-action="open-detail" title="${t("galleryDetail")}">${icons.download}</button>
            <button class="mini-link" data-action="open-settings" title="${t("gallerySettings")}">${icons.gear}</button>
            <button class="mini-link" data-action="browse" title="${t("galleryPath")}">${icons.folder}</button>
          </div>
        </div>
      </div>
    </section>
  `;
}

export function miniDock(state, download) {
  if (!state || state.completed) return "";
  if (state.status === "ready") return "";
  const hasDl = download && (download.total || 0) > 0;
  const ex = extractInfo(state);
  const pct =
    hasDl && download.total
      ? Math.min(100, (download.downloaded / download.total) * 100)
      : ex
        ? ex.pct
        : overallPercent(state);
  const running = state.status === "running" || state.status === "extracting";
  const paused = state.status === "paused";
  const label = hasDl ? tFile(download.step_id) : ex ? tStep(ex.id) : "Bloodborne";
  const meta = hasDl
    ? `${Math.floor((download.downloaded / Math.max(download.total, 1)) * 100)}% · ${formatEta(download.eta_secs)}`
    : ex && ex.cur != null
      ? `${ex.pct}% · ${ex.cur}/${ex.tot} · ${ex.left} ${t("leftUnit")}`
      : `${ex ? ex.pct : overallPercent(state)}%`;
  const showToggle = running || paused || state.status === "error";

  return `
    <aside class="mini-dock ${running ? "live" : ""} ${paused ? "paused" : ""}" data-el="mini-dock" title="${t("openDetailHint")}">
      <div class="mini-bar"><div class="mini-fill" data-el="mini-fill" style="width:${pct}%"></div></div>
      <div class="mini-row">
        <div class="mini-icon">${icons.download}</div>
        <div class="mini-copy">
          <div class="mini-title" data-el="mini-title">${label}</div>
          <div class="mini-meta" data-el="mini-meta">${meta}</div>
        </div>
        ${
          showToggle
            ? `<button class="icon-btn mini-toggle" data-action="${running ? "pause" : "resume"}" data-stop>
          ${running ? icons.pause : icons.play}
        </button>`
            : ""
        }
      </div>
    </aside>
  `;
}

export function screenPipeline(state, download) {
  const steps = (state.steps || [])
    .map((step, i) => {
      const cls = step.status || "pending";
      const icon = STEP_ICONS[step.id] || icons.download;
      return `
        <div class="step soft-step ${cls}" data-step="${step.id}" style="--i:${i}">
          <div class="step-rail"><span class="step-node"></span>${i < (state.steps.length - 1) ? '<span class="step-line"></span>' : ""}</div>
          <div class="step-icon">${icon}</div>
          <div class="step-body">
            <div class="step-title">${tStep(step.id)}</div>
            <div class="step-detail" data-el="detail">${tDetail(step.detail)}</div>
            ${
              cls === "running"
                ? `<div class="step-mini-bar"><div class="step-mini-fill" style="width:${step.percent || 0}%"></div></div>`
                : ""
            }
          </div>
          <div class="step-badge">${tBadge(cls)}</div>
        </div>
      `;
    })
    .join("");

  const statusText = tStatus(state.status);
  const isError = state.status === "error" && !!state.last_error;
  const errInfo = isError ? explainError(state.last_error) : null;
  const heading = isError ? errInfo.title : statusText;
  const subline = isError ? errInfo.body : t("detailSub");
  const downloadVisible = download && (download.total || 0) > 0;
  const ex = extractInfo(state);
  const exMeta = extractMeta(ex);
  const pct =
    downloadVisible && download.total
      ? Math.floor((download.downloaded / download.total) * 100)
      : ex
        ? ex.pct
        : overallPercent(state);
  const indeterminate = !downloadVisible && !ex && state.status === "extracting";
  const softBg = GALLERY_IMAGES[1];

  return `
    <section class="screen screen-soft" data-screen="pipeline">
      <div class="soft-bg" aria-hidden="true">
        <div class="soft-image" style="background-image:url('${softBg}')"></div>
        <div class="soft-veil"></div>
        <div class="soft-orb a"></div>
        <div class="soft-orb b"></div>
        <img class="soft-logo" src="/logo.png" alt="" draggable="false" />
      </div>

      <div class="soft-content">
        ${isError ? errorBannerHtml(state) : ""}
        <div class="row soft-head" style="justify-content:space-between;align-items:flex-start">
          <div class="title-block">
            <div class="eyebrow">${isError ? t("errTitle") : t("detailEyebrow")}</div>
            <h1 class="title display${isError ? " error-title" : ""}" data-el="pipeline-title" style="font-size:clamp(28px,4vw,40px)">${heading}</h1>
            <p class="subtitle" data-el="pipeline-sub" style="margin-top:8px">${subline}</p>
          </div>
          <div class="dock">
            <span class="pill ${state.status === "error" ? "warn" : "ok"}">
              ${state.status === "running" || state.status === "extracting" ? icons.bolt : icons.disk}
              ${statusText}
            </span>
            <button class="btn btn-ghost" data-action="open-gallery">${icons.folder} ${t("galleryBack")}</button>
          </div>
        </div>

        <div class="card pad glass-card progress-card">
          <div class="progress-wrap soft-progress">
            <div class="progress-fill ${indeterminate ? "indeterminate" : ""}" data-el="bar"
              style="width:${pct}%"></div>
          </div>
          <div class="meta-line soft-meta">
            <span class="meta-lead" data-el="file-name">${
              downloadVisible
                ? tFile(download.step_id)
                : ex
                  ? tStep(ex.id)
                  : t("ready")
            }</span>
            <span class="meta-stats">
              <span data-el="file-bytes">${
                downloadVisible
                  ? `${formatBytes(download.downloaded)} / ${formatBytes(download.total)}`
                  : exMeta.bytes
              }</span>
              <span data-el="file-speed">${downloadVisible ? formatSpeed(download.speed_bps) : exMeta.speed}</span>
              <span data-el="file-eta">${downloadVisible ? formatEta(download.eta_secs) : exMeta.eta}</span>
              <strong class="meta-pct">${pct}%</strong>
            </span>
          </div>
          <div class="dock soft-actions">
            <button class="btn btn-secondary" data-action="pause" ${state.status === "running" || state.status === "extracting" ? "" : "disabled"}>
              ${icons.pause} ${t("pause")}
            </button>
            <button class="btn btn-secondary" data-action="resume" ${state.status === "paused" || state.status === "error" ? "" : "disabled"}>
              ${icons.play} ${t("resume")}
            </button>
            <button class="btn btn-ghost" data-action="browse">${icons.folder} ${t("pathChange")}</button>
            <button class="btn btn-ghost" data-action="open-settings">${icons.gear} ${t("settingsBtn")}</button>
          </div>
        </div>

        <div class="steps soft-steps" data-el="steps">${steps}</div>

        <div class="bottom-hint">
          <span>${t("sourceNote")}</span>
          <span data-el="root-mini">${state.root || ""}</span>
        </div>
      </div>
    </section>
  `;
}

export function screenReady(state, port) {
  const slides = GALLERY_IMAGES.map(
    (url, i) =>
      `<div class="gallery-slide${i === 0 ? " active" : ""}" style="background-image:url('${url}')"></div>`
  ).join("");
  const gamePath = port?.game_dir || state?.root || "";
  const savePath = port?.user_dir || "";
  const fps = t(`fps_${port?.fps_mode || "90"}`);
  const upscaler = String(port?.upscaler || "fsr4").toUpperCase();
  const preset = t(`preset_${port?.preset ?? 1}`);
  const output = String(port?.output_res || "1920x1080").replace("x", " × ");

  return `
    <section class="screen gallery ready-gallery" data-screen="ready">
      <div class="gallery-bg">${slides}</div>
      <div class="gallery-veil ready-veil"></div>
      <div class="gallery-dots" data-el="gallery-dots">
        ${GALLERY_IMAGES.map((_, i) => `<span class="dot${i === 0 ? " active" : ""}"></span>`).join("")}
      </div>

      <div class="gallery-top ready-top">
        <div class="eyebrow">${t("readyKicker")}</div>
        <h1 class="title display gallery-title">${t("readySub")}</h1>
        <p class="subtitle gallery-sub">${t("readyPlayHint")}</p>
      </div>

      <div class="gallery-bottom ready-bottom">
        <div class="ready-play-block">
          <div class="ready-status-pill">${icons.check} ${t("readyStatus")}: ${t("readyOrb")}</div>
          <button class="play-btn display ready-play" data-action="play" data-el="play-btn">${icons.play} ${t("play")}</button>
          <div class="dock ready-actions">
            <button class="btn btn-secondary" data-action="advanced" title="${t("advancedHint")}">${icons.bolt} ${t("advancedBtn")}</button>
            <button class="btn btn-secondary" data-action="clean-temp" data-el="clean-temp">${icons.trash} ${t("cleanTempBtn")}</button>
            <button class="btn btn-secondary" data-action="open-settings">${icons.gear} ${t("settingsBtn")}</button>
            <button class="btn btn-ghost" data-action="recheck">${icons.refresh} ${t("recheck")}</button>
          </div>
        </div>

        <div class="ready-config card glass-card">
          <div class="eyebrow" style="margin-bottom:10px">${t("readyConfig")}</div>
          <div class="ready-config-rows">
            <div class="ready-config-row">
              <span class="k">${t("fpsLabel")}</span>
              <span class="v">${fps}</span>
            </div>
            <div class="ready-config-row">
              <span class="k">${t("upscalerLabel")}</span>
              <span class="v">${upscaler}</span>
            </div>
            <div class="ready-config-row">
              <span class="k">${t("presetLabel")}</span>
              <span class="v">${preset}</span>
            </div>
            <div class="ready-config-row">
              <span class="k">${t("outputLabel")}</span>
              <span class="v">${output}</span>
            </div>
          </div>
          <div class="ready-path">
            <div class="k">${t("readyGamePath")}</div>
            <div class="path-display active">${icons.folder}<span>${gamePath}</span></div>
          </div>
          <div class="ready-path">
            <div class="k">${t("readySavePath")}</div>
            <div class="path-display">${icons.folder}<span>${savePath}</span></div>
          </div>
        </div>
      </div>
    </section>
  `;
}

export function screenSettings(state, port) {
  const p = port || {
    game_dir: "",
    user_dir: "",
    fps_mode: "90",
    upscaler: "fsr4",
    preset: "1",
    output_res: "1920x1080",
  };
  const fpsOptions = ["30", "60", "90", "uncap"]
    .map(
      (v) =>
        `<option value="${v}"${String(p.fps_mode) === v ? " selected" : ""}>${t(`fps_${v}`)}</option>`
    )
    .join("");
  const upscalerOptions = [
    ["fsr4", "FSR 4"],
    ["fsr3", "FSR 3.1"],
    ["dlss", "DLSS"],
    ["taa", "TAA"],
    ["off", "Off"],
  ]
    .map(
      ([v, label]) =>
        `<option value="${v}"${String(p.upscaler) === v ? " selected" : ""}>${label}</option>`
    )
    .join("");
  const presetOptions = ["0", "1", "2", "3", "4"]
    .map(
      (v) =>
        `<option value="${v}"${String(p.preset) === v ? " selected" : ""}>${t(`preset_${v}`)}</option>`
    )
    .join("");
  const outputOptions = [
    "1280x720",
    "1920x1080",
    "2560x1440",
    "3840x2160",
    "2560x1080",
    "3440x1440",
  ]
    .map((v) => {
      const [w, h] = v.split("x");
      return `<option value="${v}"${String(p.output_res) === v ? " selected" : ""}>${w} × ${h}</option>`;
    })
    .join("");

  return `
    <section class="screen" data-screen="settings">
      <div class="title-block">
        <div class="eyebrow">${t("settingsBtn")}</div>
        <h1 class="title display" style="font-size:34px">${t("settingsTitle")}</h1>
        <p class="subtitle" style="margin-top:10px">${t("settingsSub")}</p>
      </div>

      <div class="settings-layout">
      <div class="card pad settings-panel port-panel">
        <div class="settings-section">
          <div class="settings-head">
            <span class="settings-dot safe"></span>
            <div>
              <div class="t">${t("portSettings")}</div>
              <div class="d">${t("portSettingsSub")}</div>
            </div>
          </div>

          <div class="port-form">
            <label class="field">
              <span class="field-label">${t("gameLocation")}</span>
              <span class="field-hint">${t("gameLocationSub")}</span>
              <div class="field-row">
                <input type="text" data-el="port-game-dir" value="${String(p.game_dir).replace(/"/g, "&quot;")}" spellcheck="false" />
                <button type="button" class="btn btn-secondary field-browse" data-action="browse-game-dir">${icons.folder}</button>
              </div>
            </label>

            <label class="field">
              <span class="field-label">${t("saveLocation")}</span>
              <span class="field-hint">${t("saveLocationSub")}</span>
              <div class="field-row">
                <input type="text" data-el="port-user-dir" value="${String(p.user_dir).replace(/"/g, "&quot;")}" spellcheck="false" />
                <button type="button" class="btn btn-secondary field-browse" data-action="browse-save-dir">${icons.folder}</button>
              </div>
            </label>

            <div class="field-grid">
              <label class="field">
                <span class="field-label">${t("fpsLabel")}</span>
                <select data-el="port-fps">${fpsOptions}</select>
              </label>
              <label class="field">
                <span class="field-label">${t("upscalerLabel")}</span>
                <select data-el="port-upscaler">${upscalerOptions}</select>
              </label>
              <label class="field">
                <span class="field-label">${t("presetLabel")}</span>
                <select data-el="port-preset">${presetOptions}</select>
              </label>
              <label class="field">
                <span class="field-label">${t("outputLabel")}</span>
                <select data-el="port-output">${outputOptions}</select>
              </label>
            </div>

            <div class="dock" style="margin-top:4px">
              <button class="btn btn-primary" data-action="save-port-settings">${icons.gear} ${t("savePortBtn")}</button>
            </div>
          </div>
        </div>

        <div class="settings-divider"></div>

        <div class="settings-section">
          <div class="settings-head">
            <span class="settings-dot safe"></span>
            <div>
              <div class="t">${t("safeClean")}</div>
              <div class="d">${t("safeCleanSub")}</div>
            </div>
          </div>
          <div class="settings-rows">
            <label class="check-row">
              <input type="checkbox" data-el="clean-parts" checked>
              <div>
                <div class="t">${t("partialDl")}</div>
                <div class="d">${t("partialDlSub")}</div>
              </div>
            </label>
            <label class="check-row">
              <input type="checkbox" data-el="clean-patch">
              <div>
                <div class="t">${t("patchTmp")}</div>
                <div class="d">${t("patchTmpSub")}</div>
              </div>
            </label>
            <label class="check-row">
              <input type="checkbox" data-el="clean-extractor">
              <div>
                <div class="t">${t("extractorTool")}</div>
                <div class="d">${t("extractorToolSub")}</div>
              </div>
            </label>
          </div>
        </div>

        <div class="settings-divider"></div>

        <div class="settings-section">
          <div class="settings-head">
            <span class="settings-dot danger"></span>
            <div>
              <div class="t">${t("resetSec")}</div>
              <div class="d">${t("resetSecSub")}</div>
            </div>
          </div>
          <div class="settings-rows">
            <label class="check-row danger">
              <input type="checkbox" data-el="clean-pkgs">
              <div>
                <div class="t">${t("allPkgs")}</div>
                <div class="d">${t("allPkgsSub")}</div>
              </div>
            </label>
            <label class="check-row danger">
              <input type="checkbox" data-el="clean-extracted">
              <div>
                <div class="t">${t("extractedGame")}</div>
                <div class="d">${t("extractedGameSub")}</div>
              </div>
            </label>
          </div>
        </div>
      </div>

      <div class="card pad settings-side">
        <div class="eyebrow" style="margin-bottom:10px">${t("installPath")}</div>
        <div class="path-display active">${icons.folder}<span>${state.root || t("noPath")}</span></div>
        <p class="subtitle" style="margin-top:14px;font-size:13px">${t("settingsPathNote")}</p>
        <div class="dock" style="margin-top:18px">
          <button class="btn btn-primary" data-action="clean">${icons.tray} ${t("cleanBtn")}</button>
          <button class="btn btn-secondary" data-action="back-from-settings">${icons.folder} ${t("back")}</button>
        </div>
      </div>
      </div>
    </section>
  `;
}

export function toast(kind, title, detail) {
  const stack = document.querySelector(".toast-stack") || (() => {
    const el = document.createElement("div");
    el.className = "toast-stack";
    document.body.appendChild(el);
    return el;
  })();
  const el = document.createElement("div");
  el.className = `toast ${kind}`;
  el.innerHTML = `<div class="tt">${title}</div><div class="td">${detail || ""}</div>`;
  stack.appendChild(el);
  window.setTimeout(() => {
    el.style.opacity = "0";
    el.style.transform = "translateX(24px)";
    window.setTimeout(() => el.remove(), 280);
  }, 4200);
}

export function powerModal(onChoice) {
  const back = document.createElement("div");
  back.className = "modal-back";
  back.innerHTML = `
    <div class="modal">
      <h3 class="display">${t("shutdownTitle")}</h3>
      <p>${t("shutdownQ")}</p>
      <div class="actions">
        <button class="btn btn-primary" data-choice="launcher">${icons.tray} ${t("stayLauncher")}</button>
        <button class="btn btn-secondary" data-choice="shutdown">${icons.power} ${t("shutdownPc")}</button>
        <button class="btn btn-ghost" data-choice="restart">${icons.refresh} ${t("restartPc")}</button>
      </div>
    </div>
  `;
  back.addEventListener("click", (event) => {
    const btn = event.target.closest("[data-choice]");
    if (!btn) return;
    const choice = btn.getAttribute("data-choice");
    back.remove();
    onChoice(choice);
  });
  document.body.appendChild(back);
  return back;
}

export function afterReadyPaint() {
  const play = document.querySelector(".play-btn");
  pulseReady(play);
}

export function afterPipelinePaint(container) {
  stagger(container, ".step", 50);
}

export function startGallerySlideshow() {
  stopGallerySlideshow();
  const slides = Array.from(document.querySelectorAll(".gallery-slide"));
  const dots = Array.from(document.querySelectorAll(".gallery-dots .dot"));
  if (slides.length < 2) return;
  let index = slides.findIndex((s) => s.classList.contains("active"));
  if (index < 0) index = 0;
  galleryTimer = window.setInterval(() => {
    slides[index].classList.remove("active");
    if (dots[index]) dots[index].classList.remove("active");
    index = (index + 1) % slides.length;
    slides[index].classList.add("active");
    if (dots[index]) dots[index].classList.add("active");
  }, 5500);
}

export function stopGallerySlideshow() {
  if (galleryTimer) {
    window.clearInterval(galleryTimer);
    galleryTimer = null;
  }
}

export function updateGalleryLive(state, download) {
  const screen = document.querySelector('[data-screen="gallery"]');
  if (!screen) return;

  const isError = state?.status === "error" && !!state?.last_error;
  const errInfo = isError ? explainError(state.last_error) : null;

  const statusEl = screen.querySelector(".gallery-title");
  if (statusEl) {
    statusEl.textContent = isError ? errInfo.title : tStatus(state?.status);
    statusEl.classList.toggle("error-title", isError);
  }

  const eyebrow = screen.querySelector(".gallery-top .eyebrow");
  if (eyebrow) eyebrow.textContent = isError ? t("errTitle") : t("flow");

  const subEl = screen.querySelector(".gallery-sub");
  if (subEl) subEl.textContent = isError ? errInfo.body : currentStepTitle(state);

  refreshErrorBanner(screen, state);

  const hasDl = download && (download.total || 0) > 0;
  const ex = extractInfo(state);
  const exMeta = extractMeta(ex);
  const bar = screen.querySelector('[data-el="bar"]');
  if (bar) {
    if (hasDl) {
      const pct = Math.min(100, (download.downloaded / download.total) * 100);
      bar.classList.remove("indeterminate");
      setBar(bar, pct);
    } else if (ex) {
      bar.classList.remove("indeterminate");
      setBar(bar, ex.pct);
    } else if (state?.status === "extracting") {
      bar.classList.add("indeterminate");
    } else {
      bar.classList.remove("indeterminate");
      setBar(bar, overallPercent(state));
    }
  }

  const setText = (sel, value) => {
    const el = screen.querySelector(sel);
    if (el) el.textContent = value;
  };

  setText(
    "[data-el='file-name']",
    hasDl
      ? tFile(download.step_id)
      : isError
        ? errInfo.body
        : ex
          ? tStep(ex.id)
          : currentStepTitle(state)
  );
  if (hasDl) {
    setText(
      "[data-el='file-bytes']",
      `${formatBytes(download.downloaded)} / ${formatBytes(download.total)}`
    );
    setText("[data-el='file-speed']", formatSpeed(download.speed_bps));
    setText("[data-el='file-eta']", formatEta(download.eta_secs));
  } else if (ex && !isError) {
    setText("[data-el='file-bytes']", exMeta.bytes);
    setText("[data-el='file-speed']", exMeta.speed);
    setText("[data-el='file-eta']", exMeta.eta);
  } else {
    setText("[data-el='file-bytes']", "");
    setText("[data-el='file-speed']", "");
    setText("[data-el='file-eta']", "");
  }
}

export function updateMiniDock(state, download) {
  const dock = document.querySelector(".mini-dock");
  if (!dock || !state) return;

  const hasDl = download && (download.total || 0) > 0;
  const ex = extractInfo(state);
  const pct =
    hasDl && download.total
      ? Math.min(100, (download.downloaded / download.total) * 100)
      : ex
        ? ex.pct
        : overallPercent(state);
  const running = state.status === "running" || state.status === "extracting";
  const paused = state.status === "paused";
  const showToggle = running || paused || state.status === "error";

  dock.classList.toggle("live", running);
  dock.classList.toggle("paused", paused);

  const fill = dock.querySelector('[data-el="mini-fill"]');
  if (fill) fill.style.width = `${pct}%`;

  const title = dock.querySelector('[data-el="mini-title"]');
  if (title) {
    title.textContent = hasDl
      ? tFile(download.step_id)
      : ex
        ? tStep(ex.id)
        : "Bloodborne";
  }

  const meta = dock.querySelector('[data-el="mini-meta"]');
  if (meta) {
    if (state.status === "error" && state.last_error) {
      meta.textContent = explainError(state.last_error).title;
    } else if (hasDl) {
      meta.textContent = `${Math.floor((download.downloaded / Math.max(download.total, 1)) * 100)}% · ${formatEta(download.eta_secs)}`;
    } else if (ex && ex.cur != null && ex.tot != null) {
      meta.textContent = `${ex.pct}% · ${ex.cur}/${ex.tot} · ${ex.left} ${t("leftUnit")}`;
    } else if (ex && ex.cur != null) {
      meta.textContent = `${ex.pct}% · ${ex.cur} ${t("filesUnit")}`;
    } else if (ex) {
      meta.textContent = `${ex.pct}%`;
    } else {
      meta.textContent = `${overallPercent(state)}%`;
    }
  }

  let toggle = dock.querySelector(".mini-toggle");
  if (showToggle && !toggle) {
    const row = dock.querySelector(".mini-row");
    if (row) {
      row.insertAdjacentHTML(
        "beforeend",
        `<button class="icon-btn mini-toggle" data-action="${running ? "pause" : "resume"}" data-stop></button>`
      );
      toggle = dock.querySelector(".mini-toggle");
    }
  }
  if (toggle) {
    if (!showToggle) {
      toggle.remove();
    } else {
      const canPause = running;
      toggle.disabled = !(canPause || paused || state.status === "error");
      toggle.setAttribute("data-action", canPause ? "pause" : "resume");
      toggle.innerHTML = canPause ? icons.pause : icons.play;
    }
  }
}

export function updatePipelineLive(state, download) {
  const screen = document.querySelector('[data-screen="pipeline"]');
  if (!screen) return;

  const isError = state.status === "error" && !!state.last_error;
  const errInfo = isError ? explainError(state.last_error) : null;
  const statusText = tStatus(state.status);

  const statusEl = screen.querySelector('[data-el="pipeline-title"]');
  if (statusEl) {
    statusEl.textContent = isError ? errInfo.title : statusText;
    statusEl.classList.toggle("error-title", isError);
  }

  const eyebrow = screen.querySelector(".soft-head .eyebrow");
  if (eyebrow) eyebrow.textContent = isError ? t("errTitle") : t("detailEyebrow");

  const subEl = screen.querySelector('[data-el="pipeline-sub"]');
  if (subEl) subEl.textContent = isError ? errInfo.body : t("detailSub");

  const pillEl = screen.querySelector(".soft-head .pill");
  if (pillEl) {
    pillEl.className = `pill ${state.status === "error" ? "warn" : "ok"}`;
    pillEl.innerHTML = `${
      state.status === "running" || state.status === "extracting" ? icons.bolt : icons.disk
    } ${statusText}`;
  }

  refreshErrorBanner(screen, state);

  const hasDl = download && (download.total || 0) > 0;
  const ex = extractInfo(state);
  const exMeta = extractMeta(ex);
  const pct = hasDl && download.total
    ? Math.floor((download.downloaded / download.total) * 100)
    : ex
      ? ex.pct
      : overallPercent(state);
  const bar = screen.querySelector('[data-el="bar"]');
  if (bar) {
    if (hasDl) {
      const p = (download.downloaded / download.total) * 100;
      bar.classList.remove("indeterminate");
      setBar(bar, p);
    } else if (ex) {
      bar.classList.remove("indeterminate");
      setBar(bar, ex.pct);
    } else if (state.status === "extracting") {
      bar.classList.add("indeterminate");
    } else {
      bar.classList.remove("indeterminate");
      setBar(bar, overallPercent(state));
    }
  }

  const pctEl = screen.querySelector(".meta-pct");
  if (pctEl) pctEl.textContent = `${pct}%`;

  const setText = (sel, value) => {
    const el = screen.querySelector(sel);
    if (el) el.textContent = value;
  };

  if (hasDl) {
    setText('[data-el="file-name"]', tFile(download.step_id));
    setText(
      '[data-el="file-bytes"]',
      `${formatBytes(download.downloaded)} / ${formatBytes(download.total)}`
    );
    setText('[data-el="file-speed"]', formatSpeed(download.speed_bps));
    setText('[data-el="file-eta"]', formatEta(download.eta_secs));
  } else if (ex) {
    setText('[data-el="file-name"]', tStep(ex.id));
    setText('[data-el="file-bytes"]', exMeta.bytes);
    setText('[data-el="file-speed"]', exMeta.speed);
    setText('[data-el="file-eta"]', exMeta.eta);
  } else {
    setText('[data-el="file-name"]', t("ready"));
    setText('[data-el="file-bytes"]', "");
    setText('[data-el="file-speed"]', "");
    setText('[data-el="file-eta"]', "");
  }

  const stepsRoot = screen.querySelector('[data-el="steps"]');
  if (stepsRoot && state.steps) {
    for (const step of state.steps) {
      const node = stepsRoot.querySelector(`[data-step="${step.id}"]`);
      if (!node) continue;
      node.className = `step soft-step ${step.status}`;
      const titleEl = node.querySelector(".step-title");
      if (titleEl) titleEl.textContent = tStep(step.id);
      const badge = node.querySelector(".step-badge");
      const detail = node.querySelector('[data-el="detail"]');
      if (badge) badge.textContent = tBadge(step.status);
      if (detail) detail.textContent = tDetail(step.detail || "");

      let mini = node.querySelector(".step-mini-bar");
      if (step.status === "running") {
        if (!mini) {
          const body = node.querySelector(".step-body") || node;
          body.insertAdjacentHTML(
            "beforeend",
            `<div class="step-mini-bar"><div class="step-mini-fill" data-el="mini-fill"></div></div>`
          );
          mini = node.querySelector(".step-mini-bar");
        }
        const fill = mini?.querySelector(".step-mini-fill");
        if (fill) fill.style.width = `${step.percent || 0}%`;
      } else if (mini) {
        mini.remove();
      }
    }
  }

  const pauseBtn = screen.querySelector('[data-action="pause"]');
  const resumeBtn = screen.querySelector('[data-action="resume"]');
  if (pauseBtn) {
    pauseBtn.disabled = !(state.status === "running" || state.status === "extracting");
  }
  if (resumeBtn) {
    resumeBtn.disabled = !(state.status === "paused" || state.status === "error");
  }
}

function overallPercent(state) {
  const steps = state?.steps || [];
  if (!steps.length) return 0;
  const sum = steps.reduce((acc, s) => acc + (s.percent || 0), 0);
  return Math.min(100, Math.floor(sum / steps.length));
}

// Çıkartma adımındaki canlı ilerleme: yüzde + "cur / tot" detayından kaldı sayısı
function extractInfo(state) {
  if (!state || state.status !== "extracting") return null;
  const steps = state.steps || [];
  const running = steps.find((s) => s.status === "running");
  if (!running) return null;
  const pct = Math.max(0, Math.min(100, running.percent || 0));
  const raw = String(running.detail || "").trim();
  // "123 / 420" veya "düzenleniyor · 123 / 420"
  const m = /(\d+)\s*\/\s*(\d+)/.exec(raw);
  if (m) {
    const cur = Number(m[1]);
    const tot = Number(m[2]);
    const left = Math.max(0, tot - cur);
    const phase = raw.replace(m[0], "").replace(/[·\s]+/g, " ").trim();
    return { pct, cur, tot, left, id: running.id, phase };
  }
  // stdout suskunsa FS yedeği: "1234 dosya"
  const m2 = /^(\d+)\s*dosya$/i.exec(raw);
  if (m2) {
    return { pct, cur: Number(m2[1]), tot: null, left: null, id: running.id, phase: "" };
  }
  return { pct, cur: null, tot: null, left: null, id: running.id, phase: raw };
}

function extractMeta(ex) {
  if (!ex) return { bytes: "", speed: "", eta: "" };
  const pct = `${ex.pct}%`;
  const phase = ex.phase ? tDetail(ex.phase) : "";
  if (ex.cur != null && ex.tot != null) {
    return {
      bytes: phase ? `${phase} · ${ex.cur} / ${ex.tot} ${t("filesUnit")}` : `${ex.cur} / ${ex.tot} ${t("filesUnit")}`,
      speed: phase && ex.left === 0 ? "" : `${ex.left} ${t("leftUnit")}`,
      eta: pct,
    };
  }
  if (phase) {
    return { bytes: phase, speed: "", eta: pct };
  }
  if (ex.cur != null) {
    return {
      bytes: `${ex.cur} ${t("filesUnit")}`,
      speed: "",
      eta: pct,
    };
  }
  return { bytes: pct, speed: "", eta: "" };
}

// Rust'tan gelen Türkçe detay metinlerini dile çevir
const DETAIL_MAP = {
  tr: {
    "düzenleniyor": "düzenleniyor",
    "taşınıyor": "taşınıyor",
    "arşiv düzenlendi": "arşiv düzenlendi",
  },
  en: {
    "zaten mevcut": "already present",
    başlıyor: "starting",
    çıkartılıyor: "extracting",
    birleştiriliyor: "merging",
    açılıyor: "unpacking",
    yazılıyor: "writing",
    tamam: "done",
    bitti: "done",
    doğrulanıyor: "verifying",
    yerleştiriliyor: "placing files",
    "eboot.bin mevcut": "eboot.bin present",
    "update sonrasi": "after update",
    "update uygulanmis": "update applied",
    "port mevcut": "port present",
    düzenleniyor: "tidying up",
    "taşınıyor": "moving files",
    "arşiv düzenlendi": "archive prepared",
    "exe aranıyor": "looking for exe",
    "exe bulundu": "exe found",
  },
};

function tDetail(raw) {
  if (!raw) return "";
  const lang = getLang();
  if (lang === "tr") return raw;
  return DETAIL_MAP.en[raw] || raw;
}

function currentStepTitle(state) {
  const steps = state?.steps || [];
  const running = steps.find((s) => s.status === "running");
  if (running) {
    const base = tStep(running.id);
    const detail = tDetail(running.detail || "");
    return detail ? `${base} · ${detail}` : base;
  }
  const next = steps.find((s) => s.status === "pending");
  if (next) return `${t("nextUp")} ${tStep(next.id)}`;
  return tStatus(state?.status);
}

export function bindGauge(freeBytes, needed = REQUIRED) {
  const circle = document.querySelector(".gauge .value");
  const pctEl = document.querySelector('[data-el="gauge-pct"]');
  const freeEl = document.querySelector('[data-el="free-size"]');
  const reqEl = document.querySelector('[data-el="required-size"]');
  const statusEl = document.querySelector('[data-el="disk-status"]');
  const pill = document.querySelector('[data-el="disk-pill"]');
  const circumference = 2 * Math.PI * 54;
  const need = Math.max(1, needed || REQUIRED);
  const enough = freeBytes >= need;
  const ratio = Math.max(0, Math.min(1, freeBytes / Math.max(need * 1.8, freeBytes)));
  if (reqEl) reqEl.textContent = formatBytes(need);
  if (circle) {
    circle.style.strokeDasharray = `${circumference}`;
    circle.style.strokeDashoffset = `${circumference * (1 - (enough ? 1 : ratio * 0.85))}`;
  }
  if (pctEl) {
    pctEl.textContent = enough ? t("enough") : `${Math.floor((freeBytes / need) * 100)}%`;
    pctEl.style.color = enough ? "var(--ok)" : "";
    pctEl.style.fontSize = enough ? "18px" : "";
  }
  if (freeEl) freeEl.textContent = formatBytes(freeBytes);
  if (statusEl) {
    statusEl.textContent = enough ? t("enough") : t("notEnough");
    statusEl.className = `v ${enough ? "ok" : "bad"}`;
  }
  if (pill) {
    pill.innerHTML = enough
      ? `${icons.disk} ${t("diskEnough") || t("enough")}`
      : `${icons.disk} ${t("diskFull")} ${formatBytes(freeBytes)} ${t("ofNeeded")}`;
  }
  if (circle && enough) {
    circle.style.stroke = "var(--ok)";
    circle.style.filter = "drop-shadow(0 0 8px rgba(61, 220, 151, 0.55))";
  }
  return enough;
}
