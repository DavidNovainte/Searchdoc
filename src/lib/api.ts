import { invoke } from "@tauri-apps/api/core";
import type {
  GoogleAuthStatus,
  GooglePrefsStatus,
  GoogleSyncMode,
  GoogleWatchItem,
  ImportLinksReport,
  IndexStats,
  LocalDriveInfo,
  SearchResponse,
  SearchScope,
  SearchSort,
  SourceFilter,
  SearchMode,
  SourceInfo,
  SyncReport,
  UpdateInfo,
  SyncStatus,
} from "../types";

export interface SearchDocumentsArgs {
  query: string;
  limit: number;
  offset?: number;
  sourceFilter: SourceFilter;
  mode: SearchMode;
  scope: SearchScope;
  sort: SearchSort;
  deep: boolean;
  deepDepth: number;
  deepLinkLimit: number;
  deepFetchMissing: boolean;
}

export function searchDocuments(args: SearchDocumentsArgs): Promise<SearchResponse> {
  return invoke<SearchResponse>("search_documents", {
    query: {
      query: args.query,
      limit: args.limit,
      offset: args.offset ?? 0,
      sourceKind: args.sourceFilter === "all" ? null : args.sourceFilter,
      mode: args.mode,
      scope: args.scope === "all" ? null : args.scope,
      sort: args.sort === "relevance" ? null : args.sort,
      deep: args.deep,
      deepDepth: args.deep ? args.deepDepth : 1,
      deepLinkLimit: args.deep ? args.deepLinkLimit : 12,
      deepFetchMissing: args.deep ? args.deepFetchMissing : false,
    },
  });
}

export function getDocumentPreview(documentId: string): Promise<string | null> {
  return invoke<string | null>("get_document_preview", { documentId });
}

export function listSources(): Promise<SourceInfo[]> {
  return invoke<SourceInfo[]>("list_sources");
}

export function getStats(): Promise<IndexStats> {
  return invoke<IndexStats>("get_stats");
}

export function backupIndex(): Promise<string> {
  return invoke<string>("backup_index");
}

export function optimizeIndex(): Promise<string> {
  return invoke<string>("optimize_index");
}

export function getGlobalShortcut(): Promise<string> {
  return invoke<string>("get_global_shortcut");
}

export function getNotionStatus(): Promise<boolean> {
  return invoke<boolean>("get_notion_status");
}

export function getAppVersion(): Promise<string> {
  return invoke<string>("get_app_version");
}

export function checkAppUpdate(): Promise<UpdateInfo> {
  return invoke<UpdateInfo>("check_app_update");
}

export function addNotionSource(token: string, databaseId: string): Promise<SourceInfo> {
  return invoke<SourceInfo>("add_notion_source", {
    token,
    databaseId,
  });
}

export function applyGlobalShortcut(shortcut: string): Promise<string> {
  return invoke<string>("apply_global_shortcut", { shortcut });
}

export function scheduleIndexRestore(path: string): Promise<string> {
  return invoke<string>("schedule_index_restore", { path });
}

export function addLocalFolder(path: string): Promise<SourceInfo> {
  return invoke<SourceInfo>("add_local_folder", { path });
}

export function listLocalDrives(): Promise<LocalDriveInfo[]> {
  return invoke<LocalDriveInfo[]>("list_local_drives");
}

export function removeSource(sourceId: string): Promise<void> {
  return invoke<void>("remove_source", { sourceId });
}

export function setSourceEnabled(sourceId: string, enabled: boolean): Promise<void> {
  return invoke<void>("set_source_enabled", { sourceId, enabled });
}

export function syncSource(sourceId: string): Promise<SyncReport> {
  return invoke<SyncReport>("sync_source", { sourceId });
}

export function syncAll(): Promise<SyncReport[]> {
  return invoke<SyncReport[]>("sync_all");
}

export function cancelSync(): Promise<void> {
  return invoke<void>("cancel_sync");
}

export function getSyncStatus(): Promise<SyncStatus> {
  return invoke<SyncStatus>("get_sync_status");
}

export function getGoogleAuthStatus(): Promise<GoogleAuthStatus> {
  return invoke<GoogleAuthStatus>("get_google_auth_status");
}

export function saveGoogleOAuthConfig(
  clientId: string,
  clientSecret: string,
): Promise<GoogleAuthStatus> {
  return invoke<GoogleAuthStatus>("save_google_oauth_config", { clientId, clientSecret });
}

export function connectGoogle(): Promise<GoogleAuthStatus> {
  return invoke<GoogleAuthStatus>("connect_google");
}

export function disconnectGoogle(): Promise<GoogleAuthStatus> {
  return invoke<GoogleAuthStatus>("disconnect_google");
}

export function getGooglePrefs(): Promise<GooglePrefsStatus> {
  return invoke<GooglePrefsStatus>("get_google_prefs");
}

export function setGoogleSyncMode(mode: GoogleSyncMode): Promise<GooglePrefsStatus> {
  return invoke<GooglePrefsStatus>("set_google_sync_mode", { mode });
}

export function setGoogleFolderFilter(rawText: string): Promise<GooglePrefsStatus> {
  return invoke<GooglePrefsStatus>("set_google_folder_filter", { rawText });
}

export function clearGoogleFolderFilter(): Promise<GooglePrefsStatus> {
  return invoke<GooglePrefsStatus>("clear_google_folder_filter");
}

export function listGoogleWatchlist(): Promise<GoogleWatchItem[]> {
  return invoke<GoogleWatchItem[]>("list_google_watchlist");
}

export function importGoogleLinks(rawText: string): Promise<ImportLinksReport> {
  return invoke<ImportLinksReport>("import_google_links", { rawText });
}

export function syncGoogleWatchlist(): Promise<ImportLinksReport> {
  return invoke<ImportLinksReport>("sync_google_watchlist");
}

export function removeGoogleWatchIds(ids: string[]): Promise<GoogleWatchItem[]> {
  return invoke<GoogleWatchItem[]>("remove_google_watch_ids", { ids });
}
