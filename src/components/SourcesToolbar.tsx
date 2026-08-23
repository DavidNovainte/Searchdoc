import "../styles/SourcesToolbar.css";
import type { Dispatch, SetStateAction } from "react";
import type { GoogleAuthStatus, GoogleWatchItem, IndexStats } from "../types";
import type { SettingsSection } from "../lib/format";
import { IconCloud, IconDrive, IconFolder } from "../icons";

export interface SourcesToolbarProps {
  busy: boolean;
  syncing: boolean;
  googleAuth: GoogleAuthStatus | null;
  showDrivePicker: boolean;
  setShowDrivePicker: Dispatch<SetStateAction<boolean>>;
  showDocsImport: boolean;
  setShowDocsImport: Dispatch<SetStateAction<boolean>>;
  localSourceCount: number;
  stats: IndexStats | null;
  watchlist: GoogleWatchItem[];
  handleAddFolder: () => void;
  handleOpenDrivePicker: () => void;
  openSettings: (section: SettingsSection) => void;
  handleSyncAll: () => void;
}

export function SourcesToolbar(props: SourcesToolbarProps) {
  const {
    busy, syncing, googleAuth, showDrivePicker, setShowDrivePicker,
    showDocsImport, setShowDocsImport, localSourceCount, stats, watchlist,
    handleAddFolder, handleOpenDrivePicker, openSettings, handleSyncAll,
  } = props;
  return (
<header className="page-toolbar">
  <div className="page-toolbar-stats">
    <span className="stat-pill">
      本地源 <strong>{localSourceCount}</strong>
    </span>
    <span className="stat-pill">
      本地文档 <strong>{stats?.local_doc_count ?? 0}</strong>
    </span>
    <span className="stat-pill">
      Google <strong>{stats?.google_doc_count ?? 0}</strong>
    </span>
    <span className="stat-pill">
      观察 <strong>{watchlist.length}</strong>
    </span>
  </div>
  <div className="page-toolbar-actions">
    <button
      type="button"
      className="btn ghost sm with-ico"
      disabled={busy}
      onClick={handleAddFolder}
    >
      <IconFolder size={14} />
      文件夹
    </button>
    <button
      type="button"
      className={
        showDrivePicker
          ? "btn primary sm with-ico"
          : "btn ghost sm with-ico"
      }
      disabled={busy}
      onClick={() => {
        if (showDrivePicker) {
          setShowDrivePicker(false);
          return;
        }
        void handleOpenDrivePicker();
      }}
    >
      <IconDrive size={14} />
      {showDrivePicker ? "收起" : "磁盘"}
    </button>
    <button
      type="button"
      className={
        showDocsImport
          ? "btn primary sm with-ico"
          : "btn ghost sm with-ico"
      }
      disabled={busy}
      onClick={() => {
        if (!googleAuth?.connected) {
          openSettings("google");
          return;
        }
        setShowDrivePicker(false);
        setShowDocsImport((v) => !v);
      }}
    >
      <IconCloud size={14} />
      {showDocsImport ? "收起" : "Docs"}
    </button>
    <button
      type="button"
      className="btn ghost sm"
      disabled={busy}
      onClick={() => void handleSyncAll()}
    >
      {syncing ? "同步中…" : "同步全部"}
    </button>
  </div>
</header>
  );
}
