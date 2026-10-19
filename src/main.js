import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { requestPermission } from "@tauri-apps/plugin-notification";
import { crossfade } from "./anim.js";
import { t, toggleLang } from "./i18n.js";
import {
  afterPipelinePaint,
  afterReadyPaint,
  bindGauge,
  explainError,
  miniDock,
  powerModal,
  screenGallery,
  screenPath,
  screenPipeline,
  screenReady,
  screenSettings,
  startGallerySlideshow,
  stopGallerySlideshow,
  titlebar,
  toast,
  updateGalleryLive,
  updateMiniDock,
  updatePipelineLive,
} from "./ui.js";

const DEFAULT_ROOT = "C:\\Program Files\\Bloodborne";
const app = document.getElementById("app");

const model = {
  screen: "path",
  root: null,
  state: null,
  download: {
    step_id: "",
    downloaded: 0,
    total: 0,
    speed_bps: 0,
    eta_secs: 0,
  },
  pendingRoot: null,
  port: null,
  rendering: false,
  gameRunning: false,
};

function showMini(state) {
  if (!state || state.completed) return false;
  return (
    model.screen === "gallery" ||
    model.screen === "pipeline" ||
    model.screen === "settings"
  );
}

function shell(inner, state) {
  const dock = showMini(state) ? miniDock(state, model.download) : "";
  return `<div class="shell">${titlebar()}<div class="stage">${inner}</div>${dock}</div>`;
}

async function boot() {
  try {
    const state = await invoke("get_state");
    model.state = state;
    if (state.root) {
      model.root = state.root;
      model.pendingRoot = state.root;
      if (state.completed) {
        model.screen = "ready";
        await loadPortSettings();
      } else {
        model.screen = "gallery";
      }
    } else {
      model.screen = "path";
      model.pendingRoot = DEFAULT_ROOT;
    }
  } catch (error) {
    model.screen = "path";
    model.pendingRoot = DEFAULT_ROOT;
    toast("error", t("stateErr"), String(error));
  }

  await render();
  wireEvents();
  requestPermission().catch(() => {});

  if (model.screen === "path" && model.pendingRoot) {
    await refreshDisk(model.pendingRoot);
  }

  if (model.screen === "gallery" && model.state && !model.state.completed) {
    const status = model.state.status;
    const paused = status === "paused" || model.state.user_paused === true;
    if (!paused) {
      await startPipeline();
    }
  }
}

async function refreshDisk(path) {
  try {
    const info = await invoke("disk_info", { path });
    const enough = bindGauge(info.free_bytes, info.total_required);
    const confirm = app.querySelector('[data-action="confirm-root"]');
    if (confirm) confirm.disabled = !enough;
    if (!enough) {
      toast(
        "error",
        t("noDisk"),
        `${(info.total_required / 1024 ** 3).toFixed(1)} GB / ${(info.free_bytes / 1024 ** 3).toFixed(1)} GB`
      );
    }
    return enough;
  } catch (error) {
    toast("error", t("diskErr"), String(error));
    return false;
  }
}

async function loadPortSettings() {
  try {
    model.port = await invoke("get_port_settings");
  } catch {
    model.port = null;
  }
}

function updatePlayButton() {
  const btn = document.querySelector('[data-el="play-btn"]') ||
    document.querySelector('[data-action="play"]');
  if (!btn) return;
  btn.innerHTML = model.gameRunning
    ? `${stopIcon} ${t("stop")}`
    : `${playIcon} ${t("play")}`;
  btn.classList.toggle("is-running", model.gameRunning);
}

const playIcon = `<svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M8 5.5v13l11-6.5-11-6.5z" fill="currentColor"/></svg>`;
const stopIcon = `<svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="1.5" fill="currentColor"/></svg>`;

async function render() {
  if (model.rendering) return;
  model.rendering = true;
  try {
    stopGallerySlideshow();
    let inner;
    if (model.screen === "path") {
      inner = screenPath(model.pendingRoot);
    } else if (model.screen === "gallery") {
      inner = screenGallery(model.state || { steps: [], status: "idle" }, model.download);
    } else if (model.screen === "pipeline") {
      inner = screenPipeline(model.state || { steps: [], status: "idle" }, model.download);
    } else if (model.screen === "ready") {
      inner = screenReady(model.state || {}, model.port);
    } else {
      inner = screenSettings(model.state || {}, model.port);
    }

    await crossfade(app, shell(inner, model.state));

    if (model.screen === "path" && model.pendingRoot) {
      await refreshDisk(model.pendingRoot);
    }

    if (model.screen === "gallery" || model.screen === "ready") {
      startGallerySlideshow();
    }
    if (model.screen === "pipeline") afterPipelinePaint(app);
    if (model.screen === "ready") {
      afterReadyPaint();
      updatePlayButton();
    }
  } finally {
    model.rendering = false;
  }
}

function wireEvents() {
  app.addEventListener("click", async (event) => {
    const btn = event.target.closest("[data-action]");

    const mini = event.target.closest(".mini-dock");
    if (mini) {
      const action = btn?.getAttribute("data-action");
      if (action !== "pause" && action !== "resume") {
        model.screen = "pipeline";
        await render();
        return;
      }
    }

    if (!btn) return;
    const action = btn.getAttribute("data-action");

    if (action === "toggle-lang") {
      toggleLang();
      await render();
      return;
    }

    if (action === "open-detail") {
      model.screen = "pipeline";
      await render();
      return;
    }

    if (action === "open-gallery") {
      model.screen = "gallery";
      await render();
      return;
    }

    if (action === "browse") {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t("folderPickTitle"),
        defaultPath: model.pendingRoot || model.root || DEFAULT_ROOT,
      });
      if (!selected) return;
      model.pendingRoot = selected;
      if (model.screen === "path") {
        const label = app.querySelector('[data-el="path-label"] span');
        const pathBox = app.querySelector('[data-el="path-label"]');
        if (label) label.textContent = selected;
        if (pathBox) pathBox.classList.add("active");
        const confirm = app.querySelector('[data-action="confirm-root"]');
        if (confirm) confirm.disabled = true;
        await refreshDisk(selected);
      } else {
        try {
          const state = await invoke("set_root", { path: selected });
          model.state = state;
          model.root = selected;
          toast("success", t("folderUpdated"), selected);
          await render();
        } catch (error) {
          toast("error", t("folderErr"), String(error));
        }
      }
      return;
    }

    if (action === "confirm-root") {
      if (!model.pendingRoot) return;
      btn.disabled = true;
      try {
        const state = await invoke("set_root", { path: model.pendingRoot });
        model.state = state;
        model.root = model.pendingRoot;
        model.screen = "gallery";
        await render();
        await startPipeline();
      } catch (error) {
        btn.disabled = false;
        toast("error", t("installStartErr"), String(error));
      }
      return;
    }

    if (action === "pause") {
      await invoke("pause");
      return;
    }

    if (action === "resume") {
      await invoke("resume");
      return;
    }

    if (action === "open-settings") {
      await loadPortSettings();
      model.screen = "settings";
      await render();
      return;
    }

    if (action === "back-from-settings") {
      model.screen = model.state?.completed ? "ready" : "gallery";
      await render();
      return;
    }

    if (action === "recheck") {
      if (model.root) {
        try {
          const state = await invoke("set_root", { path: model.root });
          model.state = state;
          if (!state.completed) {
            model.screen = "gallery";
            await render();
            await startPipeline();
          } else {
            model.screen = "ready";
            await render();
          }
        } catch (error) {
          toast("error", t("checkErr"), String(error));
        }
      }
      return;
    }

    if (action === "clean") {
      const deletePkgs = !!app.querySelector('[data-el="clean-pkgs"]')?.checked;
      const deleteExtracted = !!app.querySelector('[data-el="clean-extracted"]')?.checked;
      const deleteExtractor = !!app.querySelector('[data-el="clean-extractor"]')?.checked;
      const parts = !!app.querySelector('[data-el="clean-parts"]')?.checked;
      const patch = !!app.querySelector('[data-el="clean-patch"]')?.checked;

      if (deletePkgs && !window.confirm(t("confirmAllPkgs"))) {
        return;
      }
      if (deleteExtracted && !window.confirm(t("confirmExtracted"))) {
        return;
      }

      try {
        const report = await invoke("clean_leftovers", {
          deletePkgs: deletePkgs || parts,
          deleteExtracted,
          deleteExtractor,
          deletePatch: patch,
        });
        toast(
          "success",
          t("cleanDone"),
          `${report.removed.length} ${t("cleanDoneSub")}`
        );
        model.screen = deleteExtracted ? "path" : model.state?.completed ? "ready" : "gallery";
        if (deleteExtracted) {
          model.state = await invoke("get_state");
          if (!model.state.completed) model.screen = "path";
        }
        await render();
      } catch (error) {
        toast("error", t("cleanErr"), String(error));
      }
      return;
    }

    if (action === "play") {
      try {
        btn.disabled = true;
        if (model.gameRunning) {
          await invoke("stop_game");
          model.gameRunning = false;
          updatePlayButton();
          toast("success", t("gameStopped"), t("gameStoppedSub"));
        } else {
          await invoke("play");
          model.gameRunning = true;
          updatePlayButton();
          toast("success", t("gameStarted"), t("gameStartedSub"));
        }
      } catch (error) {
        toast("error", t("gameOpenErr"), String(error));
      } finally {
        const b = document.querySelector('[data-action="play"]');
        if (b) b.disabled = false;
      }
      return;
    }

    if (action === "advanced") {
      try {
        await invoke("open_advanced");
        toast("success", t("advancedStarted"), t("advancedHint"));
      } catch (error) {
        toast("error", t("advancedErr"), String(error));
      }
      return;
    }

    if (action === "clean-temp") {
      try {
        btn.disabled = true;
        const report = await invoke("clean_temp");
        toast("success", t("cleanDone"), `${report.removed.length} ${t("cleanDoneSub")}`);
      } catch (error) {
        toast("error", t("cleanTempOnlyReady"), String(error));
      } finally {
        btn.disabled = false;
      }
      return;
    }

    if (action === "browse-game-dir" || action === "browse-save-dir") {
      const isGame = action === "browse-game-dir";
      const selected = await open({
        directory: true,
        multiple: false,
        title: t("folderPickTitle"),
        defaultPath:
          (isGame
            ? app.querySelector('[data-el="port-game-dir"]')?.value
            : app.querySelector('[data-el="port-user-dir"]')?.value) || undefined,
      });
      if (!selected) return;
      const input = app.querySelector(
        isGame ? '[data-el="port-game-dir"]' : '[data-el="port-user-dir"]'
      );
      if (input) input.value = selected;
      return;
    }

    if (action === "save-port-settings") {
      btn.disabled = true;
      try {
        const settings = {
          game_dir: app.querySelector('[data-el="port-game-dir"]')?.value?.trim() || "",
          user_dir: app.querySelector('[data-el="port-user-dir"]')?.value?.trim() || "",
          fps_mode: app.querySelector('[data-el="port-fps"]')?.value || "90",
          upscaler: app.querySelector('[data-el="port-upscaler"]')?.value || "fsr4",
          preset: app.querySelector('[data-el="port-preset"]')?.value || "1",
          output_res: app.querySelector('[data-el="port-output"]')?.value || "1920x1080",
        };
        model.port = await invoke("set_port_settings", { settings });
        toast("success", t("portSaved"), t("portSavedSub"));
        await render();
      } catch (error) {
        toast("error", t("portSaveErr"), String(error));
        btn.disabled = false;
      }
      return;
    }

    if (action === "minimize") {
      await getCurrentWindow().minimize();
      return;
    }

    if (action === "tray") {
      await invoke("hide_to_tray");
      return;
    }

    if (action === "quit") {
      await invoke("quit_app");
      return;
    }
  });
}

async function startPipeline() {
  try {
    await invoke("start_pipeline");
    if (model.screen !== "gallery" && model.screen !== "pipeline") {
      model.screen = "gallery";
      await render();
    } else if (
      !document.querySelector('[data-screen="gallery"]') &&
      !document.querySelector('[data-screen="pipeline"]')
    ) {
      await render();
    }
  } catch (error) {
    const message = String(error);
    if (message.includes("zaten tamam")) {
      model.screen = "ready";
      await render();
      return;
    }
    if (message.includes("zaten calisiyor") || message.includes("zaten çalışıyor")) {
      if (model.screen !== "gallery" && model.screen !== "pipeline") {
        model.screen = "gallery";
      }
      if (
        !document.querySelector('[data-screen="gallery"]') &&
        !document.querySelector('[data-screen="pipeline"]')
      ) {
        await render();
      }
      return;
    }
    toast("error", t("installStartErr"), message);
  }
}

async function onProgress(payload) {
  if (payload && payload.download) {
    model.download = payload.download;
  } else if (payload && payload.steps) {
    // set_step/set_status snapshot'ı download taşımaz → indirme bitmiş, sıradaki faz çıkmıştır
    model.download = null;
  }
  if (payload && payload.steps) {
    const wasCompleted = model.state?.completed;
    model.state = payload;
    if (payload.completed && !wasCompleted) {
      model.screen = "ready";
      await loadPortSettings();
      await render();
      toast("success", t("installDone"), t("installDoneSub"));
      return;
    }
    if (payload.status === "ready" && payload.completed && model.screen !== "ready") {
      model.screen = "ready";
      await render();
      return;
    }
  } else if (payload && payload.status && model.state) {
    model.state = { ...model.state, ...payload };
  }

  if (model.screen === "gallery") {
    updateGalleryLive(model.state || {}, model.download);
  }
  if (model.screen === "pipeline") {
    updatePipelineLive(model.state || {}, model.download);
  }
  updateMiniDock(model.state || {}, model.download);
}

async function main() {
  await listen("pipeline://progress", (event) => {
    onProgress(event.payload);
  });
  await listen("pipeline://error", (event) => {
    const raw = String(event.payload?.message || t("errUnknown"));
    const info = explainError(raw);
    toast("error", info.title, info.body);
    // akış ekranındaysan banner + başlık da güncellensin
    if (model.state) {
      model.state = { ...model.state, status: "error", last_error: raw };
    }
    if (model.screen === "gallery") {
      updateGalleryLive(model.state || {}, model.download);
    }
    if (model.screen === "pipeline") {
      updatePipelineLive(model.state || {}, model.download);
    }
    updateMiniDock(model.state || {}, model.download);
  });
  await listen("pipeline://done", async () => {
    model.screen = "ready";
    model.state = { ...(model.state || {}), completed: true, status: "ready" };
    await loadPortSettings();
    await render();
    toast("success", t("readyToast"), t("readyToastSub"));
  });
  await listen("game://started", () => {
    model.gameRunning = true;
    updatePlayButton();
  });
  await listen("game://exited", () => {
    model.gameRunning = false;
    updatePlayButton();
    powerModal(async (choice) => {
      if (choice === "shutdown") {
        try {
          await invoke("shutdown_now");
          toast("success", t("shuttingDown"), t("shuttingDownSub"));
        } catch (error) {
          toast("error", t("shutdownErr"), String(error));
        }
      } else if (choice === "restart") {
        try {
          await invoke("restart_now");
          toast("success", t("restarting"), t("restartingSub"));
        } catch (error) {
          toast("error", t("restartErr"), String(error));
        }
      }
    });
  });

  await boot();
}

main().catch((error) => {
  toast("error", t("bootErr"), String(error));
});
