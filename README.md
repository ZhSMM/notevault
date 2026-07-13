# NoteVault

> 本地优先的笔记软件 · 数据 100% 在你硬盘上 · 双向链接 · FSRS 闪卡 · Git 同步 · AI 问答 · 静态站发布
> Tauri 2 + Rust + Vue 3 + TypeScript + SQLite (FTS5) + Shiki + Mermaid + cytoscape

![Status](https://img.shields.io/badge/status-MVP-blue)
![License](https://img.shields.io/badge/license-MIT-green)

## ✨ 核心特性

- **完全本地**：笔记就是磁盘上的 `.md` 文件，永不丢失
- **双向链接** `[[wiki]]` + 块引用 `((blk_xxx))` + 反向链接面板 + 悬停预览
- **闪卡** ：FSRS v4 间隔重复算法 · `#card` / `#card-reverse` / `#cloze` 三种标记 · 复习面板
- **AI 集成** ：Ollama 本地 / OpenAI 兼容云端 · 流式对话 · 引用来源 · **AI 自动建卡**
- **图谱视图** ：cytoscape 全局图 + 标签 / 孤立节点过滤
- **Git 同步** ：自动 commit + 可视化 log + remote push
- **静态站发布**（Quartz 风格）：一键导出 HTML 部署到 GitHub Pages
- **代码友好** ：Shiki 双主题高亮（30+ 语言） + Mermaid 图表 + 笔记大纲 TOC

## 🚀 快速开始

### 安装

```bash
# 前置：Node.js 20+ / pnpm / Rust 1.75+ / Tauri 系统依赖
pnpm install
```

### 开发

```bash
pnpm tauri dev    # 带热重载的开发模式
```

### 构建

```bash
pnpm tauri build  # 生产 build
# 产物在 src-tauri/target/release/bundle/
```

## 📦 架构

```
┌─────────────────────┐
│  Vue 3 + Pinia      │  ← UI / 编辑器 / 预览 / TOC / AI 面板
└──────────┬──────────┘
           │ Tauri IPC
┌──────────▼──────────┐
│  Rust 后端           │  ← SQLite + FTS5 + FSRS + Git + HTTP
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│  vault/             │  ← 纯文本 .md 文件（git 跟踪 / 跨设备同步）
└─────────────────────┘
```

- **数据真相**：磁盘上的 `.md` 文件，索引可重建
- **配置分离**：`.config/` 永远不进 git
- **AI 旁路**：索引独立于源文件

## ⌨️ 快捷键

| 快捷键 | 作用 |
|---|---|
| `Ctrl + P` | 打开搜索面板 |
| `Ctrl + N` | 新建笔记 |
| `Ctrl + S` | 保存 |
| `Ctrl + R` | 打开 FSRS 复习面板 |
| `Ctrl + Shift + G` | 图谱视图 |
| `Ctrl + Shift + A` | AI 助手 |
| `Ctrl + Shift + E` | 静态发布 |

## 🌐 部署静态站

把 vault 推到 GitHub，配 `Settings → Pages → GitHub Actions`，workflow 会自动 build + deploy。
详见 `sample-vault/.github/workflows/publish.yml`。

## 🤖 AI 配置

打开 AI 面板右上角 `⚙️`：

- **本地 Ollama**（推荐，0 成本）：`http://localhost:11434`
- **云端 OpenAI 兼容** ：填 base_url + api_key

## 📚 文档

- [Tauri 2](https://tauri.app/v2/)
- [Vue 3](https://vuejs.org/)
- [FSRS 算法](https://github.com/open-spaced-repetition/fsrs4anki)
- [markdown-it](https://github.com/markdown-it/markdown-it)

## 📄 License

MIT — 详见 [LICENSE](LICENSE)。
