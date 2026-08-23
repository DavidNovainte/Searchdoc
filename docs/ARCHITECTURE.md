# SearchDoc Architecture

## Overview

```text
React UI  --invoke-->  Tauri commands  -->  AppState
                                              ├─ Database (SQLite FTS5)
                                              └─ Source connectors
                                                   ├─ LocalFolderSource
                                                   └─ GoogleDocsSource (OAuth + Drive export)
```

## Indexing

- Documents are stored in `documents` with a mirrored `documents_fts` virtual table.
- CJK text is character-spaced on write (`cjk_expand`) so `unicode61` tokenization can match multi-character Chinese queries.
- Search results collapse those spaces for display.

## Sources

Each connector implements:

```rust
trait SourceConnector {
    fn kind(&self) -> SourceKind;
    fn scan(&self) -> AppResult<Vec<DocumentRecord>>;
}
```

`AppState::sync_source` scans, upserts, and deletes missing external ids for that source.

## Data location

Application data dir via `directories::ProjectDirs` (`com.SearchDoc.SearchDoc`), file `index.db`.

## Next extension points

1. Stream larger local scans in bounded batches
2. Add migration coverage for future schema changes
3. Add new source connectors only after the current sync path is stable
