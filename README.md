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
- 🌐 **多协议支持** - 兼容 OpenAI API 和 Google Gemini 原生协议
- 🎨 **隐身模式** - 答案窗口支持透明隐身样式，不干扰正常使用
- ⚡ **后台常驻** - 最小化到系统托盘，随时唤起使用
- ⌨️ **全局快捷键** - 即使在全屏应用中也能响应快捷键

## 📥 安装

从 [Releases](https://github.com/baoabaob/snip-ans/releases) 页面下载最新的 MSI 安装包，双击安装即可。

## ⌨️ 快捷键

| 功能 | 默认快捷键 | 说明 |
|------|-----------|------|
| 截图（缓存） | `Ctrl+Alt+S` | 截取屏幕并加入缓存队列 |
| 发送识别 | `Ctrl+Alt+Enter` | 将缓存的所有图片发送给 AI |
| 老板键 | `Ctrl+Shift+H` | 快速隐藏/显示答案窗口 |

> 💡 所有快捷键都可以在设置中自定义，修改后需要重启应用

## ⚙️ 配置

右键点击系统托盘图标，选择 "Settings" 打开设置界面：

### API 配置
- **API Protocol**: 选择 OpenAI Compatible 或 Google Gemini Native
- **API Endpoint**: API 端点地址
- **API Key**: 你的 API 密钥
- **Model Name**: 使用的模型名称

### 界面配置
- **Output Mode**: 输出方式（悬浮窗 / 剪贴板 / 两者都）
- **Overlay Style**: 悬浮窗样式（Normal / Stealth）
- **Font Size**: 答案文字大小

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
- 💼 远程桌面环境下使用（需调整 RDP 键盘设置）

## ⚠️ 注意事项

1. **远程桌面使用**：在 RDP 远程桌面中，需要将 "应用 Windows 组合键" 设为 "在此计算机上" 才能使用快捷键
2. **API 费用**：请注意 AI API 的调用费用，建议使用性价比高的模型
3. **合规使用**：请遵守考试规则，本软件仅供学习交流使用

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
