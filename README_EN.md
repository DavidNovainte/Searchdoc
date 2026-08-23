# SearchDoc

**Local-first cross-source full-text search** — one box to search local documents, Google Docs and Notion, with snippets, preview and one-click open of the original.

> Not Everything (that is instant filename search). SearchDoc builds a **content index**: it answers "where did I write this?"

🌐 [简体中文](README.md) | **English**

[![CI](https://github.com/DavidNovainte/Searchdoc/actions/workflows/ci.yml/badge.svg)](https://github.com/DavidNovainte/Searchdoc/actions/workflows/ci.yml)

## What it does

- **Three sources, one search box**: local files (md/txt/pdf/docx …), Google Docs (OAuth), Notion databases
- **Real content search**: SQLite FTS5 index; fuzzy/exact modes, title/body scope, relevance/mtime sort, `OR` / `-exclude` syntax
- **CJK-friendly**: tuned tokenization for mixed Chinese/Latin queries
- **Desktop resident**: system tray, configurable global hotkey, autostart, background sync notifications (only while the window is hidden)
- **Always fresh**: file-watcher incremental sync + fallback polling; searches stay responsive during long syncs (WAL reader/writer split)
- **Your data stays yours**: the index and all credentials live in your local app-data directory; no telemetry. Network is used only when you explicitly sync a cloud source or click "Check for updates"
- **Ops built in**: index backup/restore with rollback guard, FTS optimization, GitHub Releases update check

## Quick start (Windows)

**Prerequisites**: Node.js 20+ · [Rust stable](https://rustup.rs) · WebView2 (preinstalled on Win10/11)

```bash
npm install
npm run desktop   # first Rust build takes ~1-5 minutes
```

1. On the search page click "Add folder & start" → pick a notes folder (try the bundled `fixtures/notes`)
2. Once indexing finishes, type any keyword to get highlighted snippets
3. Connecting Google / Notion is optional — see "Cloud sources" below

<details>
<summary>Common launch problems</summary>

| Symptom | Cause & fix |
|------|-----------|
| `Port 1420 is already in use` | A previous instance is still alive — **closing the window only hides it to the tray**. Quit from the tray menu or kill leftover processes. `npm run dev` ships a port pre-check |
| Build seems stuck | A cold Rust build is normal; incremental builds take seconds |
| cargo error `os error 32/5` | The freshly built exe is locked by an old instance or antivirus: quit the tray instance first |

</details>

## Cloud sources (optional)

- **Google Docs**: create a desktop OAuth client per [docs/GOOGLE_SETUP.md](docs/GOOGLE_SETUP.md) → Settings → connect your Google account → add doc links or folder filters on the Sources page
- **Notion**: create an Integration at [notion.so/my-integrations](https://www.notion.so/my-integrations) and "connect" it to your database → Settings → Sync → paste the token and database ID

## Daily operations

| Action | How |
|------|------|
| Bring up | Global hotkey (default `Ctrl+Shift+Space`) or tray icon |
| Close window | Hides to tray (does not quit) |
| Add library | Sources page: folder / drive / Docs / Notion |
| Sync | Sidebar "Sync all" (cancellable while running) |

## Architecture

```text
src/                      React UI (components by responsibility, hooks for logic)
src-tauri/src/
  lib.rs                  Tauri command registration & plugin setup
  app_state.rs            App state / sync orchestration / status broadcast
  db.rs                   SQLite FTS5 (WAL writer + dedicated read-only connection)
  models.rs               Shared models (DocumentRecord / SourceKind …)
  watcher.rs              File watching (notify + quiet-period debounce)
  update.rs               GitHub Releases update check
  shortcut_prefs.rs       Global hotkey persistence
  sources/
    mod.rs                SourceConnector trait + hashing helpers
    local.rs              Local scan (walkdir + rayon parallel parsing)
    google_docs.rs        Drive list/export with 429 backoff retry
    notion.rs             Notion database query + block flattening
```

Every source writes through the same `SourceConnector::scan() -> Vec<DocumentRecord>` contract into one shared index — adding a source means implementing a single connector.

## Development

```bash
npm test                                        # frontend unit tests
cargo test --lib --manifest-path src-tauri/Cargo.toml   # Rust unit tests
```

CI enforces: fmt · clippy (`-D warnings`) · tests on both sides · RustSec dependency audit (see [.github/workflows/ci.yml](.github/workflows/ci.yml)). Release process: [docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md).

## Privacy

SearchDoc contains no telemetry or analytics. There are exactly three kinds of network requests, all user-initiated: ① syncing Google Docs; ② syncing Notion; ③ manually clicking "Check for updates". The index database and every credential never leave your machine.

## License

[MIT](LICENSE)