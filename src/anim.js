import { t } from "./i18n.js";

export function crossfade(app, html) {
  return new Promise((resolve) => {
    app.classList.add("leaving");
    window.setTimeout(() => {
      app.innerHTML = html;
      app.classList.remove("leaving");
      app.classList.add("entering");
      window.setTimeout(() => {
        app.classList.remove("entering");
        resolve();
      }, 420);
    }, 180);
  });
}

export function formatBytes(bytes) {
  if (!bytes || Number.isNaN(bytes)) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  return `${value.toFixed(value >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatSpeed(bps) {
  if (!bps) return "0 B/s";
  return `${formatBytes(bps)}/s`;
}

export function formatEta(secs) {
  if (!secs || secs <= 0 || Number.isNaN(secs)) return "—";
  const sn = t("sn");
  const min = t("min");
  const hour = t("hour");
  if (secs < 45) return `${Math.ceil(secs / 5) * 5} ${sn}`;
  if (secs < 90) return `1 ${min}`;
  const m = Math.floor(secs / 60);
  const s = Math.round((secs % 60) / 15) * 15;
  if (m < 60) return s >= 60 ? `${m + 1} ${min}` : `${m} ${min}`;
  const h = Math.floor(m / 60);
  const rem = m % 60;
  if (rem < 5) return `${h} ${hour}`;
  return `${h} ${hour} ${rem} ${min}`;
}

export function setBar(el, percent) {
  if (!el) return;
  const next = Math.max(0, Math.min(100, Number(percent) || 0));
  el.style.width = `${next}%`;
}

export function stagger(container, selector, delay = 55) {
  const nodes = container.querySelectorAll(selector);
  nodes.forEach((node, index) => {
    node.style.animationDelay = `${index * delay}ms`;
    node.classList.remove("stagger-in");
    void node.offsetWidth;
    node.classList.add("stagger-in");
  });
}

export function pulseReady(el) {
  if (!el) return;
  el.classList.remove("ready-pulse");
  void el.offsetWidth;
  el.classList.add("ready-pulse");
}
