import "../styles/SettingsView.css";
import { useEffect, useState } from "react";
import type { Dispatch, SetStateAction } from "react";
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart";
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../lib/api";
import type {
  GoogleAuthStatus,
  GooglePrefsStatus,
  GoogleSyncMode,
  IndexStats,
} from "../types";
import type { SettingsSection } from "../lib/format";
import { SettingsNav } from "./SettingsNav";
import type { SettingsNavProps } from "./SettingsNav";
import { GoogleSection } from "./GoogleSection";
import type { GoogleSectionProps } from "./GoogleSection";
import { SyncSection } from "./SyncSection";
import type { SyncSectionProps } from "./SyncSection";
import { SystemSection } from "./SystemSection";
import type { SystemSectionProps } from "./SystemSection";
import { DataSection } from "./DataSection";
import type { DataSectionProps } from "./DataSection";
import { AboutSection } from "./AboutSection";

export interface SettingsViewProps {
  settingsSection: SettingsSection;
  setSettingsSection: Dispatch<SetStateAction<SettingsSection>>;
  stats: IndexStats | null;
  googleAuth: GoogleAuthStatus | null;
  googlePrefs: GooglePrefsStatus | null;
  setGoogleAuth: (value: GoogleAuthStatus) => void;
  setGooglePrefs: (value: GooglePrefsStatus) => void;
  flash: (message: string) => void;
  setError: (message: string | null) => void;
  setStatus: (message: string) => void;
  refreshMeta: () => Promise<void>;
}

export function SettingsView(props: SettingsViewProps) {
  const {
    settingsSection, setSettingsSection,
    stats, googleAuth, googlePrefs, setGoogleAuth, setGooglePrefs,
    flash, setError, setStatus, refreshMeta,
  } = props;

  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [settingsBusy, setSettingsBusy] = useState(false);
  const [folderFilterText, setFolderFilterText] = useState("");
  const [autostartOn, setAutostartOn] = useState(false);
  const [autostartBusy, setAutostartBusy] = useState(false);
  const [backupBusy, setBackupBusy] = useState(false);
  const [restoreBusy, setRestoreBusy] = useState(false);
  const [optimizeBusy, setOptimizeBusy] = useState(false);
  const [shortcutInput, setShortcutInput] = useState("");
  const [shortcutBusy, setShortcutBusy] = useState(false);
  const [notionToken, setNotionToken] = useState("");
  const [notionDatabaseId, setNotionDatabaseId] = useState("");
  const [notionConnected, setNotionConnected] = useState(false);
  const [notionBusy, setNotionBusy] = useState(false);

  useEffect(() => {
    isAutostartEnabled()
      .then(setAutostartOn)
      .catch(() => setAutostartOn(false));
    api
      .getGlobalShortcut()
      .then(setShortcutInput)
      .catch(() => {});
    api
      .getNotionStatus()
      .then(setNotionConnected)
      .catch(() => setNotionConnected(false));
  }, []);

  async function handleToggleAutostart() {
    try {
      setAutostartBusy(true);
      if (autostartOn) {
        await disableAutostart();
        setAutostartOn(false);
        flash("已关闭开机自启");
      } else {
        await enableAutostart();
        setAutostartOn(true);
        flash("已开启开机自启（启动后驻留托盘，可在系统设置中自定义全局快捷键）");
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setAutostartBusy(false);
    }
  }

  async function handleBackupIndex() {
    try {
      setBackupBusy(true);
      const path = await api.backupIndex();
      flash(`索引备份已创建 · ${path}`);
    } catch (err) {
      setError(String(err));
    } finally {
      setBackupBusy(false);
    }
  }

  async function handleOptimizeIndex() {
    try {
      setOptimizeBusy(true);
      const message = await api.optimizeIndex();
      flash(message);
    } catch (err) {
      setError(String(err));
    } finally {
      setOptimizeBusy(false);
    }
  }

  async function handleApplyShortcut() {
    try {
      setShortcutBusy(true);
      const applied = await api.applyGlobalShortcut(shortcutInput);
      setShortcutInput(applied);
      flash(`全局快捷键已更新为 ${applied}`);
    } catch (err) {
      setError(String(err));
    } finally {
      setShortcutBusy(false);
    }
  }

  async function handleAddNotionSource() {
    try {
      setNotionBusy(true);
      const info = await api.addNotionSource(notionToken.trim(), notionDatabaseId.trim());
      setNotionConnected(true);
      setNotionToken("");
      setNotionDatabaseId("");
      flash(`Notion 来源「${info.name}」已添加 · 去「来源」页点同步拉取内容`);
      void refreshMeta();
    } catch (err) {
      setError(String(err));
    } finally {
      setNotionBusy(false);
    }
  }

  async function handleRestoreIndex() {
    try {
      const selectedPath = await open({
        multiple: false,
        title: "选择 SearchDoc 索引备份",
        filters: [{ name: "SQLite", extensions: ["db", "sqlite", "sqlite3"] }],
      });
      if (!selectedPath || Array.isArray(selectedPath)) return;
      if (!window.confirm("恢复将在下次启动时生效。当前索引会保留为回滚备份，继续吗？")) return;
      setRestoreBusy(true);
      await api.scheduleIndexRestore(selectedPath);
      flash("恢复已准备好 · 完全退出并重新启动 SearchDoc 后生效");
    } catch (err) {
      setError(String(err));
    } finally {
      setRestoreBusy(false);
    }
  }

  async function handleSaveGoogleConfig() {
    try {
      setSettingsBusy(true);
      setStatus("正在保存 Google OAuth 配置…");
      const next = await api.saveGoogleOAuthConfig(clientId, clientSecret);
      setGoogleAuth(next);
      setClientSecret("");
      flash("Google OAuth 配置已保存");
      await refreshMeta();
    } catch (err) {
      setError(String(err));
    } finally {
      setSettingsBusy(false);
    }
  }

  async function handleConnectGoogle() {
    try {
      setSettingsBusy(true);
      setStatus("请在浏览器完成 Google 授权…");
      const next = await api.connectGoogle();
      setGoogleAuth(next);
      await refreshMeta();
      flash(
        next.connected ? "Google 已连接" : "Google 授权流程结束",
      );
    } catch (err) {
      setError(String(err));
    } finally {
      setSettingsBusy(false);
    }
  }

  async function handleDisconnectGoogle() {
    try {
      setSettingsBusy(true);
      setStatus("正在断开 Google…");
      const next = await api.disconnectGoogle();
      setGoogleAuth(next);
      await refreshMeta();
      flash("已断开 Google，并清理云端索引");
    } catch (err) {
      setError(String(err));
    } finally {
      setSettingsBusy(false);
    }
  }

  async function handleSetGoogleSyncMode(mode: GoogleSyncMode) {
    try {
      setSettingsBusy(true);
      const next = await api.setGoogleSyncMode(mode);
      setGooglePrefs(next);
      flash(
        mode === "watchlist_only"
          ? "已切换为：仅观察列表（同步不会全盘扫描）"
          : "已切换为：最近文档浅同步",
      );
    } catch (err) {
      setError(String(err));
    } finally {
      setSettingsBusy(false);
    }
  }

  async function handleSaveFolderFilter() {
    try {
      setSettingsBusy(true);
      const next = await api.setGoogleFolderFilter(folderFilterText);
      setGooglePrefs(next);
      setFolderFilterText("");
      const n = next.folder_ids?.length ?? 0;
      flash(
        n > 0
          ? `已保存文件夹筛选：${n} 个（仅「最近文档」模式生效，仅直接子文档）`
          : "已清空文件夹筛选（最近文档将扫描全盘浅列表）",
      );
    } catch (err) {
      setError(String(err));
    } finally {
      setSettingsBusy(false);
    }
  }

  async function handleClearFolderFilter() {
    try {
      setSettingsBusy(true);
      const next = await api.clearGoogleFolderFilter();
      setGooglePrefs(next);
      setFolderFilterText("");
      flash("已清除 Google 文件夹筛选");
    } catch (err) {
      setError(String(err));
    } finally {
      setSettingsBusy(false);
    }
  }

  async function handleRemoveOneFolder(folderId: string) {
    try {
      setSettingsBusy(true);
      const remain = (googlePrefs?.folder_ids ?? []).filter((id) => id !== folderId);
      const next = await api.setGoogleFolderFilter(remain.join("\n"));
      setGooglePrefs(next);
      flash(remain.length ? `已移除文件夹，剩余 ${remain.length} 个` : "已清除全部文件夹筛选");
    } catch (err) {
      setError(String(err));
    } finally {
      setSettingsBusy(false);
    }
  }

  const settingsNavProps: SettingsNavProps = {
    settingsSection, setSettingsSection, googleAuth, googlePrefs, autostartOn,
  };
  const googleSectionProps: GoogleSectionProps = {
    googleAuth, clientId, setClientId, clientSecret, setClientSecret,
    googleBusy: settingsBusy,
    handleConnectGoogle, handleDisconnectGoogle, handleSaveGoogleConfig,
  };
  const syncSectionProps: SyncSectionProps = {
    googlePrefs, googleBusy: settingsBusy, folderFilterText, setFolderFilterText,
    handleSetGoogleSyncMode, handleClearFolderFilter, handleRemoveOneFolder, handleSaveFolderFilter,
    notionToken, setNotionToken, notionDatabaseId, setNotionDatabaseId,
    notionConnected, notionBusy, handleAddNotionSource,
  };
  const systemSectionProps: SystemSectionProps = {
    autostartOn, autostartBusy, handleToggleAutostart,
    optimizeBusy, handleOptimizeIndex,
    shortcutInput, setShortcutInput, shortcutBusy, handleApplyShortcut,
  };
  const dataSectionProps: DataSectionProps = {
    stats, googleAuth, backupBusy, handleBackupIndex, restoreBusy, handleRestoreIndex,
  };

  return (
    <div className="settings-shell">
      <SettingsNav {...settingsNavProps} />
      <section className="panel settings-detail">
        {settingsSection === "google" && <GoogleSection {...googleSectionProps} />}
        {settingsSection === "sync" && <SyncSection {...syncSectionProps} />}
        {settingsSection === "system" && <SystemSection {...systemSectionProps} />}
        {settingsSection === "data" && <DataSection {...dataSectionProps} />}
        {settingsSection === "about" && <AboutSection />}
      </section>
    </div>
  );
}
