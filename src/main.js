const { invoke } = window.__TAURI__.core; // Usage for tauri v2 usually window.__TAURI__.core.invoke
// Or window.__TAURI__.invoke if using older binding patterns.
// With withGlobalTauri: true in v2, the global object structure is tricky.
// Usually `const { invoke } = window.__TAURI__.core;` is correct for V2.

const statusEl = document.getElementById("status");
const fields = ["api_key", "api_endpoint", "hotkey", "boss_key", "send_key", "output_mode", "prompt", "model_name", "api_protocol", "overlay_style", "font_size"];

async function loadSettings() {
  try {
    const config = await invoke("get_config");
    console.log("Config loaded:", config);
    fields.forEach(field => {
      const el = document.getElementById(field);
      if (el) el.value = config[field] || "";
    });
  } catch (e) {
    console.error("Failed to load config:", e);
    statusEl.textContent = "Error loading config: " + e;
  }
}

async function saveSettings() {
  try {
    const config = {};
    fields.forEach(field => {
      const value = document.getElementById(field).value;
      // Convert font_size to number
      if (field === "font_size") {
        config[field] = parseInt(value, 10) || 14;
      } else {
        config[field] = value;
      }
    });

    await invoke("update_config", { config });

    // Notify overlay of style change
    await invoke("notify_overlay_style", { style: config.overlay_style });

    // Notify overlay of font size change
    await invoke("notify_font_size", { size: config.font_size });

    statusEl.textContent = "Settings saved!";
    statusEl.style.color = "#4caf50";
    setTimeout(() => { statusEl.textContent = ""; }, 3000);
  } catch (e) {
    console.error("Failed to save config:", e);
    statusEl.textContent = "Error saving: " + e;
    statusEl.style.color = "#f44336";
  }
}

document.getElementById("save").addEventListener("click", saveSettings);
window.addEventListener("DOMContentLoaded", loadSettings);

const { listen } = window.__TAURI__.event;
const logEl = document.getElementById("logs");

listen('log-message', (event) => {
  const entry = document.createElement('div');
  entry.textContent = `[${new Date().toLocaleTimeString()}] ${event.payload}`;
  logEl.appendChild(entry);
  logEl.scrollTop = logEl.scrollHeight;
});
