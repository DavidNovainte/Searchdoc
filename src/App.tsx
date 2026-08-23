import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  GoogleAuthStatus,
  GooglePrefsStatus,
  IndexStats,
  LocalDriveInfo,
  SyncReport,
  SyncStatus,
  SyncSummary,
} from "./types";
import * as api from "./lib/api";
import { listen } from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { debugLog } from "./lib/debugLog";
import {
  isWindowsDriveRoot,
} from "./lib/format";
import type { SettingsSection, Tab } from "./lib/format";
import { Sidebar } from "./components/Sidebar";
import type { SidebarProps } from "./components/Sidebar";
import { AppHeader } from "./components/AppHeader";
import type { AppHeaderProps } from "./components/AppHeader";
import { SearchView } from "./components/SearchView";
import type { SearchViewProps } from "./components/SearchView";
import { SourcesView } from "./components/SourcesView";
import type { SourcesViewProps } from "./components/SourcesView";
import { SettingsView } from "./components/SettingsView";
import type { SettingsViewProps } from "./components/SettingsView";
import "./styles/base.css";
import "./styles/App.css";
import "./styles/shared.css";
import "./styles/responsive.css";


const ONBOARDING_DONE_KEY = "searchdoc.onboardingDone";

/** Best-effort OS notification; only fires while the window is hidden so
 * active users are never interrupted. */
async function notifyIfHidden(title: string, body: string): Promise<void> {
  try {
    if (!document.hidden) return;
    let granted = await isPermissionGranted();
    if (!granted) granted = (await requestPermission()) === "granted";
    if (granted) sendNotification({ title, body });
  } catch {
    // Notifications must never break the status flow.
  }
}
const SIDEBAR_COLLAPSED_KEY = "searchdoc.sidebarCollapsed";

function loadSidebarCollapsed(): boolean {
  try {
    return localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === "1";
  } catch {
    return false;
  }
}


export default function App() {
  const [tab, setTab] = useState<Tab>("search");
  const [settingsReturnTab, setSettingsReturnTab] = useState<Tab>("search");
  const [settingsSection, setSettingsSection] =
    useState<SettingsSection>("google");
  const [stats, setStats] = useState<IndexStats | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const lastSyncLog = useRef("");
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState("准备就绪");
  const [googleAuth, setGoogleAuth] = useState<GoogleAuthStatus | null>(null);
  const [googlePrefs, setGooglePrefs] = useState<GooglePrefsStatus | null>(null);
  const [bootstrapping, setBootstrapping] = useState(true);
  const [success, setSuccess] = useState<string | null>(null);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(loadSidebarCollapsed);
  function toggleSidebarCollapsed() {
    setSidebarCollapsed((prev) => {
      const next = !prev;
      try {
        localStorage.setItem(SIDEBAR_COLLAPSED_KEY, next ? "1" : "0");
      } catch {
        /* ignore */
      }
      return next;
    });
  }
  const [syncPhase, setSyncPhase] = useState<string | null>(null);
  const [syncSummary, setSyncSummary] = useState<SyncSummary | null>(null);

  // Index/sync ops only — do not lock actions while debounced search is loading.
  const backgroundSyncing = syncStatus?.running ?? false;
  const indexBusy = syncing || backgroundSyncing;
  const busy = indexBusy;

  useEffect(() => {
    let active = true;
    let prevRunning = false;
    const process = (next: SyncStatus) => {
      if (!active) return;
      // Running→done while hidden (tray / watcher / auto-sync) ⇒ notify once.
      if (
        prevRunning &&
        !next.running &&
        (next.phase === "completed" || next.phase === "partial") &&
        document.hidden
      ) {
        void notifyIfHidden(
          "SearchDoc 同步完成",
          `更新 ${next.indexed} · 移除 ${next.removed}` +
            (next.errors > 0 ? ` · ${next.errors} 个问题` : ""),
        );
      }
      prevRunning = next.running;
      const syncKey = JSON.stringify([
        next.running, next.phase, next.source_id, next.visited,
        next.processed, next.indexed, next.removed, next.errors,
        next.error_kind,
      ]);
      if (syncKey !== lastSyncLog.current) {
        lastSyncLog.current = syncKey;
        debugLog("sync.status", next);
        setSyncStatus(next);
      }
      const phase = {
        preparing: "准备同步",
        scanning: "扫描文件",
        indexing: "写入索引",
        completed: "同步完成",
        partial: "同步完成，但有问题",
        cancelled: "同步已取消",
        failed: "同步失败",
        idle: "准备就绪",
      }[next.phase] || next.phase;
      const counts = next.running && (next.visited || next.scanned)
        ? ` (${next.processed}/${next.scanned || "?"})`
        : "";
      setSyncPhase(next.source_name ? `${phase} · ${next.source_name}${counts}` : `${phase}${counts}`);
    };
    // Primary channel: backend pushes every status transition over IPC.
    const unlisten = listen<SyncStatus>("sync-status", (event) => process(event.payload));
    // Initial load plus a slow safety net in case an event is ever missed.
    void api.getSyncStatus().then(process).catch(() => undefined);
    const timer = window.setInterval(() => {
      void api.getSyncStatus().then(process).catch(() => undefined);
    }, 60000);
    return () => {
      active = false;
      window.clearInterval(timer);
      void unlisten.then((fn) => fn());
    };
  }, []);


  const refreshMeta = useCallback(async () => {
    const [nextStats, nextGoogle, nextPrefs] = await Promise.all([
      api.getStats(),
      api.getGoogleAuthStatus(),
      api.getGooglePrefs(),
    ]);
    setStats(nextStats);
    setGoogleAuth(nextGoogle);
    setGooglePrefs(nextPrefs);
  }, []);

  useEffect(() => {
    refreshMeta()
      .catch((err) => setError(String(err)))
      .finally(() => setBootstrapping(false));
  }, [refreshMeta]);


  function openSettings(section: SettingsSection = "google") {
    setSettingsReturnTab(tab === "settings" ? settingsReturnTab : tab);
    setSettingsSection(section);
    setTab("settings");
  }

  function leaveSettings() {
    setTab(settingsReturnTab === "settings" ? "search" : settingsReturnTab);
  }

  // Focus search box when entering search tab.



  async function addLocalPath(path: string, labelHint?: string) {
    const drive = isWindowsDriveRoot(path);
    if (drive) {
      const label = labelHint || path;
      const ok = window.confirm(
        `将索引整盘「${label}」。\n\n` +
          `• 仅处理文档类文件（md / txt / pdf / docx 等）\n` +
          `• 自动跳过 Windows、Program Files 等系统目录\n` +
          `• 首次可能较久，期间可点「取消同步」\n\n` +
          `确定继续？`,
      );
      if (!ok) return;
    }

    setSyncing(true);
    setSyncPhase(drive ? "1/2 扫描磁盘文件" : "1/2 扫描文件夹");
    setStatus(
      labelHint
        ? `索引 ${labelHint}${drive ? " · 首次可能较久，可取消" : ""}`
        : "建立本地正文索引…",
    );
    try {
      await api.addLocalFolder(path);
      await refreshMeta();
      try {
        localStorage.setItem(ONBOARDING_DONE_KEY, "1");
      } catch {
        // Onboarding persistence is optional; the source was added successfully.
      }
      flash(
        labelHint
          ? `${labelHint} 已加入，可以开始搜索`
          : "已加入本地库，可以开始搜索",
      );
      setTab("search");
    } catch (err) {
      const msg = String(err);
      if (msg.includes("已取消")) {
        flash("已取消索引");
      } else {
        setError(msg);
      }
    } finally {
      setSyncing(false);
      setSyncPhase(null);
    }
  }

  async function handleAddFolder() {
    try {
      const selectedPath = await open({
        directory: true,
        multiple: false,
        title: "选择要搜索的文件夹",
      });
      if (!selectedPath || Array.isArray(selectedPath)) return;
      await addLocalPath(selectedPath);
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleCancelSync() {
    try {
      await api.cancelSync();
      setSyncPhase("正在取消…");
      setStatus("正在取消…");
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleAddDrive(drive: LocalDriveInfo) {
    await addLocalPath(drive.path, drive.label);
  }

  async function handleSyncOne(sourceId: string) {
    try {
      setSyncing(true);
      let label = "来源";
      try {
        const list = await api.listSources();
        const src = list.find((s) => s.id === sourceId);
        if (src) label = src.kind === "google_docs" ? "Google Docs" : src.name || "来源";
      } catch {
        /* fallback label */
      }
      setSyncPhase(`同步 ${label}`);
      setStatus(`正在同步：${label}`);
      const report = await api.syncSource(sourceId);
      await refreshMeta();
      setSyncSummary({
        label: `${label} 同步完成`,
        at: Date.now(),
        indexed: report.indexed,
        removed: report.removed,
        skipped: report.skipped,
        errors: report.errors,
      });
      flash(
        `同步完成：更新 ${report.indexed}，未变 ${report.skipped}，移除 ${report.removed}${
          report.errors.length ? `，问题 ${report.errors.length}` : ""
        }`,
      );
    } catch (err) {
      const msg = String(err);
      if (msg.includes("已取消")) {
        flash("已取消同步");
        await refreshMeta().catch(() => undefined);
      } else {
        setError(msg);
      }
    } finally {
      setSyncing(false);
      setSyncPhase(null);
    }
  }


  async function handleSyncAll() {
    try {
      setSyncing(true);
      const srcs = await api.listSources();
      const enabled = srcs.filter((s) => s.enabled);
      if (enabled.length === 0) {
        flash("没有已启用的来源");
        return;
      }

      setSyncPhase("同步全部来源");
      setStatus("正在同步正文索引…");
      const reports: SyncReport[] = await api.syncAll();

      await refreshMeta();
      const total = reports.reduce((sum, r) => sum + r.indexed, 0);
      const skipped = reports.reduce((sum, r) => sum + r.skipped, 0);
      const removed = reports.reduce((sum, r) => sum + r.removed, 0);
      const problems = reports.reduce((sum, r) => sum + r.errors.length, 0);
      setSyncSummary({
        label: problems ? "同步完成，但有问题" : "全部来源同步完成",
        at: Date.now(),
        indexed: total,
        removed,
        skipped,
        errors: reports.flatMap((r) => r.errors),
      });
      flash(
        problems
          ? `同步完成，但有 ${problems} 个问题 · 变更 ${total} · 跳过 ${skipped}`
          : `正文索引已更新 · 变更 ${total} · 未变 ${skipped}`,
      );
    } catch (err) {
      const msg = String(err);
      if (msg.includes("已取消")) {
        flash("已取消同步");
        await refreshMeta().catch(() => undefined);
      } else {
        setError(msg);
      }
    } finally {
      setSyncing(false);
      setSyncPhase(null);
    }
  }






  useEffect(() => {
    if (!success) return;
    const t = window.setTimeout(() => setSuccess(null), 4200);
    return () => window.clearTimeout(t);
  }, [success]);

  function flash(message: string) {
    // Soft errors (partial sync) should not hide the success summary.
    setError(null);
    setSuccess(message);
    setStatus(message);
  }

  const inSettings = tab === "settings";

  const searchViewProps: SearchViewProps = {
    active: tab === "search",
    stats, indexBusy, setTab,
    handleAddFolder, handleSyncAll,
    flash, setStatus, setError,
  };
  const sourcesViewProps: SourcesViewProps = {
    stats, syncing, busy, googleAuth, googlePrefs,
    handleAddFolder, handleAddDrive,
    handleSyncAll, handleSyncOne, openSettings,
    flash, setError, setStatus, setSyncPhase, refreshMeta,
    syncSummary, onDismissSyncSummary: () => setSyncSummary(null),
  };
  const settingsViewProps: SettingsViewProps = {
    settingsSection, setSettingsSection,
    stats, googleAuth, googlePrefs, setGoogleAuth, setGooglePrefs,
    flash, setError, setStatus, refreshMeta,
  };

  const sidebarProps: SidebarProps = {
    tab, setTab, collapsed: sidebarCollapsed, onToggleSidebar: toggleSidebarCollapsed,
    indexBusy, busy, status, syncPhase, syncing: indexBusy,
    googleAuth, bootstrapping, stats,
    handleCancelSync, handleSyncAll, openSettings,
  };
  const appHeaderProps: AppHeaderProps = {
    tab, inSettings, settingsSection, leaveSettings,
    success, setSuccess, error, setError,
  };

  return (
    <div
      className={`app-shell${sidebarCollapsed ? " sidebar-collapsed" : ""}${
        inSettings ? " settings-mode" : ""
      }`}
    >
      {!inSettings && <Sidebar {...sidebarProps} />}

      <div className="main">
      <AppHeader {...appHeaderProps} />

        <div
          className={
            tab === "settings" || tab === "sources"
              ? "content content-flush"
              : "content"
          }
        >
          {bootstrapping ? (
            <div className="boot-panel panel">
              <div className="spinner lg" />
              <h3>正在加载 SearchDoc…</h3>
              <p>读取索引库与来源状态</p>
            </div>
          ) : (
            <>
              <div className="view" hidden={tab !== "search"}>
                <SearchView {...searchViewProps} />
              </div>
              <div className="view" hidden={tab !== "sources"}>
                <SourcesView {...sourcesViewProps} />
              </div>
              <div className="view" hidden={tab !== "settings"}>
                <SettingsView {...settingsViewProps} />
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
