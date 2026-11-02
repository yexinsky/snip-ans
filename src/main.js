const { invoke } = window.__TAURI__.core;

const statusEl = document.getElementById("status");
const providerEl = document.getElementById("provider");
const providerHintEl = document.getElementById("provider-hint");
const modelSelectEl = document.getElementById("model_select");
const modelInputEl = document.getElementById("model_name");
const endpointEl = document.getElementById("api_endpoint");
const apiKeyEl = document.getElementById("api_key");
const protocolEl = document.getElementById("api_protocol");
const fetchModelsBtn = document.getElementById("fetch-models");
const logEl = document.getElementById("logs");

// 保存到后端的字段（provider 单独处理，见 saveSettings）
const fields = [
  "api_key",
  "api_endpoint",
  "hotkey",
  "boss_key",
  "send_key",
  "output_mode",
  "prompt",
  "model_name",
  "api_protocol",
  "overlay_style",
  "font_size",
];

let providers = [];
let lastProvider = null;

function setStatus(text, ok) {
  statusEl.textContent = text;
  statusEl.style.color = ok ? "#4caf50" : "#f44336";
  if (ok) {
    setTimeout(() => {
      if (statusEl.textContent === text) statusEl.textContent = "";
    }, 3000);
  }
}

function appendLog(text) {
  const entry = document.createElement("div");
  entry.textContent = `[${new Date().toLocaleTimeString()}] ${text}`;
  logEl.appendChild(entry);
  logEl.scrollTop = logEl.scrollHeight;
}

function currentPreset() {
  return providers.find((p) => p.id === providerEl.value) || null;
}

/** 切换供应商时，套用它的默认端点/协议/模型。 */
function applyPresetDefaults(preset) {
  if (!preset) return;
  providerHintEl.textContent = preset.hint || "";

  if (preset.default_endpoint) {
    endpointEl.value = preset.default_endpoint;
  } else {
    endpointEl.value = "";
  }
  protocolEl.value = preset.protocol;
  modelInputEl.value = preset.default_model || "";

  // 端点/模型允许手改；不可改写的场景目前没有，保留开关以备扩展
  endpointEl.disabled = !preset.editable_endpoint;
  resetModelOptions();
}

function resetModelOptions() {
  modelSelectEl.innerHTML = '<option value="">（点右侧按钮获取模型列表）</option>';
}

async function loadProviders() {
  try {
    providers = await invoke("get_providers");
    providerEl.innerHTML = "";
    providers.forEach((p) => {
      const opt = document.createElement("option");
      opt.value = p.id;
      opt.textContent = p.label;
      providerEl.appendChild(opt);
    });
  } catch (e) {
    appendLog("加载供应商列表失败：" + e);
  }
}

async function loadSettings() {
  try {
    await loadProviders();
    const config = await invoke("get_config");
    console.log("已加载配置", config);

    fields.forEach((field) => {
      const el = document.getElementById(field);
      if (el) el.value = config[field] ?? "";
    });

    // 供应商：老配置可能没有该字段，回落到 openai
    providerEl.value = config.provider || "openai";
    lastProvider = providerEl.value;
    const preset = currentPreset();
    if (preset) {
      providerHintEl.textContent = preset.hint || "";
      endpointEl.disabled = !preset.editable_endpoint;
    }

    // 模型下拉里放上当前已保存的模型，便于识别
    if (config.model_name) {
      const opt = document.createElement("option");
      opt.value = config.model_name;
      opt.textContent = config.model_name + "（已保存）";
      modelSelectEl.appendChild(opt);
      modelSelectEl.value = config.model_name;
    }
  } catch (e) {
    console.error("加载配置失败", e);
    setStatus("加载配置失败：" + e, false);
  }
}

async function saveSettings() {
  try {
    const config = {};
    fields.forEach((field) => {
      const value = document.getElementById(field).value;
      config[field] = field === "font_size" ? parseInt(value, 10) || 14 : value;
    });
    config.provider = providerEl.value;

    await invoke("update_config", { config });
    await invoke("notify_overlay_style", { style: config.overlay_style });
    await invoke("notify_font_size", { size: config.font_size });

    setStatus("设置已保存", true);
  } catch (e) {
    console.error("保存配置失败", e);
    setStatus("保存失败：" + e, false);
  }
}

async function fetchModels() {
  const provider = providerEl.value;
  const endpoint = endpointEl.value.trim();
  const apiKey = apiKeyEl.value.trim();

  if (!endpoint) {
    setStatus("请先填写 API 端点", false);
    return;
  }
  if (!apiKey) {
    setStatus("请先填写 API Key", false);
    return;
  }

  fetchModelsBtn.disabled = true;
  const originalText = fetchModelsBtn.textContent;
  fetchModelsBtn.textContent = "获取中…";
  setStatus("正在从上游获取模型列表…", true);

  try {
    const models = await invoke("fetch_models", {
      provider,
      endpoint,
      apiKey,
    });

    modelSelectEl.innerHTML = "";
    models.forEach((m) => {
      const opt = document.createElement("option");
      opt.value = m.id;
      opt.textContent = m.owned_by ? `${m.id}（${m.owned_by}）` : m.id;
      modelSelectEl.appendChild(opt);
    });

    // 若当前手填的模型在列表里，保持选中；否则选第一个
    const current = modelInputEl.value.trim();
    const hit = models.find((m) => m.id === current);
    if (hit) {
      modelSelectEl.value = hit.id;
    } else {
      modelSelectEl.value = models[0].id;
      modelInputEl.value = models[0].id;
    }

    setStatus(`已获取 ${models.length} 个模型`, true);
    appendLog(`已从上游获取 ${models.length} 个模型`);
  } catch (e) {
    setStatus("获取模型失败", false);
    appendLog("获取模型失败：" + e);
  } finally {
    fetchModelsBtn.disabled = false;
    fetchModelsBtn.textContent = originalText;
  }
}

document.getElementById("save").addEventListener("click", saveSettings);
fetchModelsBtn.addEventListener("click", fetchModels);

// 切换下拉里的模型时，同步到手填输入框
modelSelectEl.addEventListener("change", () => {
  if (modelSelectEl.value) modelInputEl.value = modelSelectEl.value;
});

// 切换供应商时套用默认值（不覆盖用户已保存的当前供应商）
providerEl.addEventListener("change", () => {
  const preset = currentPreset();
  applyPresetDefaults(preset);
  lastProvider = providerEl.value;
});

window.addEventListener("DOMContentLoaded", loadSettings);

const { listen } = window.__TAURI__.event;

listen("log-message", (event) => {
  appendLog(event.payload);
});
