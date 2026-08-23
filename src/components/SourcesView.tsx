import "../styles/SourcesView.css";
import { useEffect, useMemo, useState } from "react";
import * as api from "../lib/api";
import type {
  GoogleAuthStatus,
  GooglePrefsStatus,
  GoogleWatchItem,
  IndexStats,
  LocalDriveInfo,
  SourceInfo,
  SyncSummary,
} from "../types";
import type { SettingsSection } from "../lib/format";
import { SourcesToolbar } from "./SourcesToolbar";
import type { SourcesToolbarProps } from "./SourcesToolbar";
import { DrivePicker } from "./DrivePicker";
import type { DrivePickerProps } from "./DrivePicker";
import { SourceList } from "./SourceList";
import type { SourceListProps } from "./SourceList";
import { DocsImport } from "./DocsImport";
import type { DocsImportProps } from "./DocsImport";
import { WatchList } from "./WatchList";
import type { WatchListProps } from "./WatchList";

export interface SourcesViewProps {
  stats: IndexStats | null;
  syncing: boolean;
  busy: boolean;
  googleAuth: GoogleAuthStatus | null;
  googlePrefs: GooglePrefsStatus | null;
  handleAddFolder: () => void;
  handleAddDrive: (drive: LocalDriveInfo) => void;
  handleSyncAll: () => void;
  handleSyncOne: (id: string) => void;
  openSettings: (section: SettingsSection) => void;
  flash: (message: string) => void;
  setError: (message: string | null) => void;
  setStatus: (message: string) => void;
  setSyncPhase: (value: string | null) => void;
  refreshMeta: () => Promise<void>;
  syncSummary: SyncSummary | null;
  onDismissSyncSummary: () => void;
}

export function SourcesView(props: SourcesViewProps) {
  const {
    stats, syncing, busy, googleAuth, googlePrefs,
    handleAddFolder, handleAddDrive,
    handleSyncAll, handleSyncOne, openSettings,
    flash, setError, setStatus, setSyncPhase, refreshMeta,
    syncSummary, onDismissSyncSummary,
  } = props;

  const [sources, setSources] = useState<SourceInfo[]>([]);
  const [watchlist, setWatchlist] = useState<GoogleWatchItem[]>([]);
  const [localDrives, setLocalDrives] = useState<LocalDriveInfo[]>([]);
  const [showDrivePicker, setShowDrivePicker] = useState(false);
  const [showDocsImport, setShowDocsImport] = useState(false);
  const [linkText, setLinkText] = useState("");
  const [docsBusy, setDocsBusy] = useState(false);

  const localSourceCount = useMemo(
    () => sources.filter((s) => s.kind === "local").length,
    [sources],
  );

  async function refreshLocal() {
    const [srcs, watch] = await Promise.all([
      api.listSources(),
      api.listGoogleWatchlist().catch(() => [] as GoogleWatchItem[]),
    ]);
    setSources(srcs);
    setWatchlist(watch);
  }

  useEffect(() => {
    refreshLocal().catch((err) => setError(String(err)));
  }, []);

  async function handleOpenDrivePicker() {
    try {
      setShowDocsImport(false);
      const drives = await api.listLocalDrives();
      setLocalDrives(drives);
      setShowDrivePicker(true);
      if (drives.length === 0) {
        flash("未检测到可用磁盘，请改用「+ 本地文件夹」");
      }
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleAddDriveWrapped(drive: LocalDriveInfo) {
    setShowDrivePicker(false);
    await handleAddDrive(drive);
  }

  async function handleToggle(source: SourceInfo) {
    try {
      await api.setSourceEnabled(source.id, !source.enabled);
      await Promise.all([refreshLocal(), refreshMeta()]);
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleRemove(sourceId: string) {
    try {
      await api.removeSource(sourceId);
      await Promise.all([refreshLocal(), refreshMeta()]);
      flash("已移除本地源");
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleImportLinks() {
    try {
      setDocsBusy(true);
      setSyncPhase("解析 Docs 链接…");
      setStatus("正在解析并同步 Docs 链接…");
      const report = await api.importGoogleLinks(linkText);
      setWatchlist(report.watchlist);
      setLinkText("");
      await Promise.all([refreshLocal(), refreshMeta()]);
      const invalidNote = report.invalid.length
        ? `，无法解析 ${report.invalid.length}`
        : "";
      const errNote = report.errors.length ? `，错误 ${report.errors.length}` : "";
      flash(
        `链接导入：新增 ${report.added}，已有 ${report.already_tracked}，更新 ${report.updated}，跳过 ${report.skipped}${invalidNote}${errNote}`,
      );
      if (report.errors.length) {
        setStatus(report.errors.slice(0, 2).join("；"));
      } else {
        setShowDocsImport(false);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setDocsBusy(false);
      setSyncPhase(null);
    }
  }

  async function handleSyncWatchlist() {
    try {
      setDocsBusy(true);
      setSyncPhase("同步观察列表…");
      setStatus("正在同步观察列表…");
      const report = await api.syncGoogleWatchlist();
      setWatchlist(report.watchlist);
      await Promise.all([refreshLocal(), refreshMeta()]);
      flash(
        `观察列表同步：更新 ${report.updated}，跳过 ${report.skipped}${
          report.errors.length ? `，错误 ${report.errors.length}` : ""
        }`,
      );
      if (report.errors.length) {
        setStatus(report.errors.slice(0, 2).join("；"));
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setDocsBusy(false);
      setSyncPhase(null);
    }
  }

  async function handleRemoveWatch(id: string) {
    try {
      setDocsBusy(true);
      const next = await api.removeGoogleWatchIds([id]);
      setWatchlist(next);
      await Promise.all([refreshLocal(), refreshMeta()]);
      flash("已从观察列表移除，并清理对应索引");
    } catch (err) {
      setError(String(err));
    } finally {
      setDocsBusy(false);
    }
  }

  const toolbarProps: SourcesToolbarProps = {
    busy, syncing, googleAuth, showDrivePicker, setShowDrivePicker,
    showDocsImport, setShowDocsImport, localSourceCount, stats, watchlist,
    handleAddFolder, openSettings, handleOpenDrivePicker, handleSyncAll,
  };
  const drivePickerProps: DrivePickerProps = {
    localDrives, sources, busy, setShowDrivePicker, handleAddDrive: handleAddDriveWrapped,
  };
  const sourceListProps: SourceListProps = {
    sources, googleAuth, googlePrefs, syncing, googleBusy: docsBusy, busy,
    handleAddFolder, openSettings, handleSyncOne, handleToggle, handleRemove,
  };
  const docsImportProps: DocsImportProps = {
    linkText, setLinkText, googleBusy: docsBusy, setShowDocsImport, handleImportLinks,
  };
  const watchListProps: WatchListProps = {
    watchlist, googleBusy: docsBusy, googleAuth, handleSyncWatchlist, handleRemoveWatch,
  };

  return (
    <div className="sources-shell">
      <section className="panel sources-panel">
        <SourcesToolbar {...toolbarProps} />
        {syncSummary && (
          <div
            className={`sync-summary${syncSummary.errors.length ? " has-errors" : ""}`}
            role="status"
          >
            <div className="sync-summary-head">
              <div>
                <strong>{syncSummary.label}</strong>
                <span className="sync-summary-time">
                  {new Date(syncSummary.at).toLocaleTimeString()}
                </span>
              </div>
              <button
                type="button"
                className="sync-summary-close"
                onClick={onDismissSyncSummary}
                aria-label="关闭同步摘要"
                title="关闭"
              >
                ×
              </button>
            </div>
            <div className="sync-summary-counts">
              <span>更新 {syncSummary.indexed}</span>
              <span>未变 {syncSummary.skipped}</span>
              <span>移除 {syncSummary.removed}</span>
              {syncSummary.errors.length > 0 && (
                <span className="sync-summary-problems">
                  问题 {syncSummary.errors.length}
                </span>
              )}
            </div>
            {syncSummary.errors.length > 0 && (
              <ul className="sync-summary-errors">
                {syncSummary.errors.slice(0, 6).map((e, i) => (
                  <li key={i} title={e}>
                    {e}
                  </li>
                ))}
                {syncSummary.errors.length > 6 && (
                  <li className="sync-summary-more">
                    … 还有 {syncSummary.errors.length - 6} 条
                  </li>
                )}
              </ul>
            )}
          </div>
        )}
        {showDrivePicker && <DrivePicker {...drivePickerProps} />}
        <SourceList {...sourceListProps} />
        {showDocsImport && googleAuth?.connected && <DocsImport {...docsImportProps} />}
        {watchlist.length > 0 && <WatchList {...watchListProps} />}
      </section>
    </div>
  );
}