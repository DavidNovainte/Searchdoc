export type SourceKind = "local" | "google_docs" | "notion";

export type SourceFilter = "all" | "local" | "google_docs" | "notion";

export interface UpdateInfo {
  current: string;
  latest: string | null;
  updateAvailable: boolean;
  releaseUrl: string | null;
  notes: string | null;
  disabled: boolean;
}
export type SearchMode = "fuzzy" | "exact";
export type SearchScope = "all" | "title" | "body";
export type SearchSort = "relevance" | "mtime";

export interface SourceInfo {
  id: string;
  kind: SourceKind;
  name: string;
  root_path: string | null;
  enabled: boolean;
  last_sync_at: string | null;
  doc_count: number;
}

export interface SearchHit {
  id: string;
  source_id: string;
  source_kind: SourceKind;
  title: string;
  uri: string;
  snippet: string;
  rank: number;
  mtime: string | null;
  depth?: number;
  via?: string | null;
}

export interface DeepSearchStats {
  enabled: boolean;
  max_depth: number;
  seeds: number;
  links_found: number;
  links_followed: number;
  fetched: number;
  linked_hits: number;
  skipped_existing: number;
  errors: string[];
}

export interface SearchResponse {
  hits: SearchHit[];
  deep: DeepSearchStats;
}

export interface IndexStats {
  document_count: number;
  source_count: number;
  local_doc_count: number;
  google_doc_count: number;
  db_path: string;
}

export interface SyncReport {
  source_id: string;
  indexed: number;
  removed: number;
  skipped: number;
  errors: string[];
}

export interface SyncStatus {
  running: boolean;
  phase: "idle" | "preparing" | "scanning" | "indexing" | "completed" | "partial" | "cancelled" | "failed";
  source_id: string | null;
  source_name: string | null;
  visited: number;
  scanned: number;
  processed: number;
  indexed: number;
  removed: number;
  skipped: number;
  errors: number;
  error_kind: SyncErrorKind | null;
  message: string | null;
}

export type SyncErrorKind =
  | "access_denied"
  | "network"
  | "parse"
  | "missing_source"
  | "cancelled"
  | "unknown";

export interface SyncSummary {
  label: string;
  at: number;
  indexed: number;
  removed: number;
  skipped: number;
  errors: string[];
}

export interface LocalDriveInfo {
  path: string;
  label: string;
}

export interface GoogleAuthStatus {
  configured: boolean;
  connected: boolean;
  has_refresh_token: boolean;
  source_id: string;
  config_path: string;
  token_path: string;
}

export type GoogleSyncMode = "recent" | "watchlist_only";

export interface GoogleFolderItem {
  id: string;
  name: string | null;
}

export interface GooglePrefsStatus {
  sync_mode: GoogleSyncMode;
  sync_mode_label: string;
  folder_ids: string[];
  folders: GoogleFolderItem[];
  prefs_path: string;
}

export interface GoogleWatchItem {
  id: string;
  url: string;
  title: string | null;
  added_at: string;
}

export interface ImportLinksReport {
  parsed: number;
  added: number;
  already_tracked: number;
  invalid: string[];
  fetched: number;
  updated: number;
  skipped: number;
  errors: string[];
  watchlist: GoogleWatchItem[];
}
