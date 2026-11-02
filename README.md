# SnipAns 📸🤖

**一款轻量级的 AI 截图搜题助手** - 一键截图，智能识题，快速获取答案。

![Platform](https://img.shields.io/badge/Platform-Windows-blue)
![License](https://img.shields.io/badge/License-CC%20BY--NC--SA%204.0-green)
![Built with](https://img.shields.io/badge/Built%20with-Tauri%20%2B%20Rust-orange)

## ✨ 主要功能

### 🎯 核心功能
- **一键截图识题** - 全局快捷键截取屏幕，自动发送给 AI 进行题目识别
- **多图批量处理** - 支持缓存最多 8 张截图，一次性发送给 AI 批量处理
- **智能答案展示** - 答案通过置顶悬浮窗展示，支持 Markdown 和 LaTeX 数学公式渲染
- **老板键隐藏** - 一键快速隐藏/显示答案窗口

### 🔧 特性
- 🖼️ **图片自动压缩** - 截图自动缩放至 1280px 并使用 JPEG 压缩，节省 API 调用成本
- 🌐 **多供应商支持** - 内置 OpenAI、DeepSeek 官方、OpenDesign、Google Gemini 与自定义网关
- 🔎 **一键获取模型** - 从上游 `/models` 接口拉取可用模型列表，直接下拉选择
- 🎨 **隐身模式** - 答案窗口支持透明隐身样式，不干扰正常使用
- ⚡ **后台常驻** - 最小化到系统托盘，随时唤起使用
- ⌨️ **全局快捷键** - 即使在全屏应用中也能响应快捷键

## 📥 安装

从 [Releases](https://github.com/baoabaob/snip-ans/releases) 页面下载最新的 MSI 安装包，双击安装即可。

## ⚙️ 配置

右键点击系统托盘图标，选择「设置」打开设置界面。

### 模型服务

设置界面把模型相关配置集中在一处：

| 项目 | 说明 |
|------|------|
| 供应商 | 内置 OpenAI、DeepSeek 官方、OpenDesign、Google Gemini、自定义网关 |
| API Key | 对应供应商的密钥，输入框为密码类型 |
| API 端点 | 完整的对话接口地址（含 `chat/completions` 或 `generateContent`） |
| 模型 | 「获取模型」按钮会拉取上游列表供下拉选择，也可直接手填模型名 |
| 协议 | OpenAI 兼容 / Google Gemini 原生，切换供应商时自动匹配 |

切换供应商会自动填入该预设的默认端点、协议与模型；已经手填过的字段仍可继续覆盖。

### 各供应商接入方式

- **OpenAI**：填入 `https://api.openai.com/v1/chat/completions` 与平台 Key，模型拉取自 `/v1/models`。
- **DeepSeek 官方**：填入 `https://api.deepseek.com/chat/completions` 与在 [DeepSeek 开放平台](https://platform.deepseek.com/) 申请的 Key；模型列表来自 `https://api.deepseek.com/models`，常见模型有 `deepseek-chat`、`deepseek-reasoner`。
- **OpenDesign**：内置官方 Link 网关，默认端点已填好，选用后只需填 Key：
  - **API 端点**：`https://amr-link.open-design.ai/v1/chat/completions`（默认值）
  - **API Key**：`profiles.prod.runtimeKey`，取自本机 `~/.amr/config.json`
  - 若使用自建隧道，把端点换成 `<linkUrl>/v1/chat/completions` 即可，模型列表会跟着走同一主机。
  - 默认模型 `deepseek-v4.1-flash`，支持图片输入，适合截图识别。

  实测该端点可返回 13 个模型，其中 `claude-haiku-5.5`、`deepseek-v4.1-flash`、`glm-5.3-flash`、`gpt-6-luna`、`mimo-v2.6-pro` 等 10 个支持图片输入。
  因模型清单与账号权限相关，只有「获取模型」真正返回了非空列表，才算该供应商配置成功。
- **Google Gemini**：使用原生协议，认证头为 `x-goog-api-key`，模型列表解析自 `{ "models": [...] }`。
- **自定义网关**：任意 OpenAI 兼容中转，端点留空时由「获取模型」按 `/chat/completions` 自动推导 `/models`。

> 推理模型（如 `deepseek-reasoner`）会先花 token 思考。若答案被截断，会提示提高「最大输出 Token」，而不是抛出无法解析的错误。

### 界面与输出

- **输出方式**：悬浮窗 / 仅剪贴板 / 两者都
- **悬浮窗样式**：普通（深色背景）/ 隐身（透明背景、黑字）
- **字号**：悬浮窗文字大小，范围 10–32 像素

### 快捷键

| 功能 | 默认快捷键 | 说明 |
|------|-----------|------|
| 截图（缓存） | `Ctrl+Alt+S` | 截取屏幕并加入缓存队列，最多 8 张 |
| 发送识别 | `Ctrl+Alt+Enter` | 将缓存中的所有图片发送给 AI |
| 老板键 | `Ctrl+Shift+H` | 快速隐藏/显示答案窗口 |

> 💡 快捷键修改后需要重启应用；若组合重复或被其他程序占用，启动时会写日志并弹出设置窗口提示。

## 🔨 从源码构建

### 环境要求
- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) 1.70+
- [Tauri CLI](https://v2.tauri.app/start/prerequisites/)

### 构建步骤

```bash
# 克隆仓库
git clone https://github.com/baoabaob/snip-ans.git
cd snip-ans

# 安装依赖
npm install

# 开发模式运行
npm run tauri dev

# 构建发布版本
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/msi/`

## 📝 使用场景

- 📚 在线考试时快速查询答案
- 📖 学习时遇到难题快速获取解题思路

⚠️ 请遵守考试规则，本软件仅供学习交流使用

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

本项目采用 [CC BY-NC-SA 4.0](LICENSE) 许可证。

- ✅ 允许自由分发和修改
- ✅ 需要注明原作者
- ❌ **禁止商业使用**
- 📝 修改后需使用相同许可证

---

**Made with ❤️ using Tauri + Rust**
