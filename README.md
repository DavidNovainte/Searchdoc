# SearchDoc

**本地优先的跨源正文搜索** —— 统一检索本地文档、Google Docs 与 Notion，命中即给片段、预览与原文直达。

> 不是 Everything（那是文件名秒搜）。SearchDoc 建的是**正文索引**：回答「这句话写在哪」。

[English](#english) · [中文](#中文)

[![CI](https://github.com/DavidNovainte/Searchdoc/actions/workflows/ci.yml/badge.svg)](https://github.com/DavidNovainte/Searchdoc/actions/workflows/ci.yml)

---

## 中文

### 它能做什么

- **统一搜索三个来源**：本地文件（md/txt/pdf/docx 等）、Google Docs（OAuth）、Notion 数据库
- **真·正文检索**：SQLite FTS5 全文索引；模糊/精准模式、标题/正文范围、相关度/最新排序、`OR` / `-排除` 语法
- **中文友好**：CJK 分词特调，中英混排查询实测可用
- **桌面常驻**：系统托盘、全局快捷键（可在设置中改键）、开机自启、后台同步完成通知（窗口隐藏时才打扰）
- **自动保持新鲜**：文件监听增量同步 + 兜底轮询，同步期间搜索照常可用（WAL 读写分离）
- **数据自主**：索引与凭据全部在本机应用数据目录；无遥测。联网仅发生在你主动同步 Google/Notion 或点击「检查更新」时
- **运维齐备**：索引备份/恢复（带回滚保护）、FTS 碎片整理、发布版更新检查（GitHub Releases）

### 5 分钟上手（Windows）

**依赖**：Node.js 20+ · [Rust stable](https://rustup.rs) · WebView2（Win10/11 通常自带）

```bash
npm install
npm run desktop   # 首次编译约 1~5 分钟
```

1. 搜索页「添加文件夹并开始」→ 选一个笔记目录（仓库内 `fixtures/notes` 可试用）
2. 首次索引完成后输入关键词即可搜到片段
3. 连接 Google / Notion 是可选增强，见下方「云源接入」

<details>
<summary>运行失败的常见原因</summary>

| 现象 | 原因与处理 |
|------|-----------|
| `Port 1420 is already in use` | 上次实例没退出——**关窗只是隐藏到托盘**。托盘右键「退出」，或结束残留进程。`npm run dev` 已内置端口预检 |
| 编译卡很久 | Rust 冷编译正常现象 |
| cargo 报 `os error 32/5` | 新 exe 被旧实例或杀软锁定：先退托盘实例再试 |

</details>

### 云源接入（可选）

- **Google Docs**：按 [docs/GOOGLE_SETUP.md](docs/GOOGLE_SETUP.md) 创建桌面 OAuth 客户端 → 设置 → Google 账号连接 → 来源页加链接或配置文件夹筛选
- **Notion**：在 [notion.so/my-integrations](https://www.notion.so/my-integrations) 创建 Integration 并在目标数据库「连接」它 → 设置 → 同步 → 粘贴 Token 与数据库 ID

### 日常操作

| 操作 | 方式 |
|------|------|
| 唤起 | 全局快捷键（默认 `Ctrl+Shift+Space`）或托盘图标 |
| 关窗 | 隐藏到托盘（不退出） |
| 加库 | 来源页：文件夹 / 磁盘 / Docs / Notion |
| 同步 | 侧栏「同步全部」（进行中可取消） |

### 架构

```text
src/                      React UI（组件按职责拆分，hooks 承载逻辑）
src-tauri/src/
  lib.rs                  Tauri commands 注册与插件装配
  app_state.rs            应用状态 / 同步编排 / 状态广播
  db.rs                   SQLite FTS5（WAL 主写 + 只读连接分担查询）
  models.rs               共享模型（DocumentRecord / SourceKind …）
  watcher.rs              文件监听（notify + 静默期防抖）
  update.rs               GitHub Releases 更新检查
  shortcut_prefs.rs       全局快捷键持久化
  sources/
    mod.rs                SourceConnector trait + 哈希工具
    local.rs              本地扫描（walkdir + rayon 并行解析）
    google_docs.rs        Drive 列表/导出 + 429 退避重试
    notion.rs             Notion 数据库查询 + 块展平
```

所有来源经同一 `SourceConnector::scan() -> Vec<DocumentRecord>` 写入共享索引——新增来源只需实现一个 connector。

### 开发

```bash
npm test                                        # 前端单测
cargo test --lib --manifest-path src-tauri/Cargo.toml   # Rust 单测
```

CI 强制：fmt · clippy(`-D warnings`) · 双端测试 · RustSec 依赖审计（见 [.github/workflows/ci.yml](.github/workflows/ci.yml)）。发布流程见 [docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md)。

### 隐私声明

SearchDoc 不含任何遥测/埋点。网络请求仅有三类且全部由你触发：① 同步 Google Docs；② 同步 Notion；③ 手动点击「检查更新」。索引数据库与所有凭据只存本机应用数据目录。

### License

[MIT](LICENSE)

---

## English

**Local-first cross-source full-text search** — one box to search local documents, Google Docs and Notion, with snippets, preview and one-click open.

Highlights:

- Real **content** search over SQLite FTS5 (fuzzy/exact modes, title/body scope, relevance/mtime sort, boolean syntax), tuned for mixed CJK/Latin queries
- Three pluggable sources behind a single `SourceConnector` trait: local folders (parallel parsing via rayon, mtime fast-path), Google Drive export with 429 backoff, Notion databases with block flattening
- Tray-resident desktop app: configurable global hotkey, autostart, background sync notifications (only while hidden), file-watcher incremental sync + fallback polling
- Privacy by design: no telemetry; network is used only when you sync a cloud source or click "check for updates"; index and credentials never leave your machine
- Ops built in: index backup/restore with rollback guard, FTS optimization, GitHub Releases update check

**Build (Windows):** Node 20+, Rust stable, WebView2 → `npm install && npm run desktop`. Tests: `npm test`, `cargo test --lib --manifest-path src-tauri/Cargo.toml`.

Licensed under [MIT](LICENSE). Chinese documentation above is the canonical, more detailed one.
