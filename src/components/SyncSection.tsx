import "../styles/SyncSection.css";
import type { Dispatch, SetStateAction } from "react";
import type { GooglePrefsStatus, GoogleSyncMode } from "../types";

export interface SyncSectionProps {
  googlePrefs: GooglePrefsStatus | null;
  googleBusy: boolean;
  folderFilterText: string;
  setFolderFilterText: Dispatch<SetStateAction<string>>;
  handleSetGoogleSyncMode: (mode: GoogleSyncMode) => void;
  handleClearFolderFilter: () => void;
  handleRemoveOneFolder: (id: string) => void;
  handleSaveFolderFilter: () => void;
  notionToken: string;
  setNotionToken: (value: string) => void;
  notionDatabaseId: string;
  setNotionDatabaseId: (value: string) => void;
  notionConnected: boolean;
  notionBusy: boolean;
  handleAddNotionSource: () => void;
}

export function SyncSection(props: SyncSectionProps) {
  const {
    googlePrefs, googleBusy, folderFilterText, setFolderFilterText,
    handleSetGoogleSyncMode, handleClearFolderFilter, handleRemoveOneFolder, handleSaveFolderFilter,
    notionToken, setNotionToken, notionDatabaseId, setNotionDatabaseId,
    notionConnected, notionBusy, handleAddNotionSource,
  } = props;
  return (
<div className="settings-detail-body">
  <header className="settings-detail-head">
    <div>
      <h2>同步范围</h2>
      <p>决定「同步全部」会扫哪些 Google Docs</p>
    </div>
  </header>

  <div className="settings-card tight">
    <div className="settings-row">
      <div className="settings-row-text">
        <strong>模式</strong>
        <span>
          {googlePrefs?.sync_mode === "recent"
            ? "最近 Docs，可再限文件夹"
            : "只同步「来源」里添加的链接"}
        </span>
      </div>
    </div>
    <div
      className="seg-control"
      role="tablist"
      aria-label="Google 同步范围"
    >
      <button
        type="button"
        className={
          googlePrefs?.sync_mode === "watchlist_only"
            ? "seg active"
            : "seg"
        }
        disabled={googleBusy}
        onClick={() =>
          void handleSetGoogleSyncMode("watchlist_only")
        }
      >
        观察列表
      </button>
      <button
        type="button"
        className={
          googlePrefs?.sync_mode === "recent"
            ? "seg active"
            : "seg"
        }
        disabled={googleBusy}
        onClick={() => void handleSetGoogleSyncMode("recent")}
      >
        最近文档
      </button>
    </div>
  </div>

  <div
    className={
      googlePrefs?.sync_mode === "recent"
        ? "settings-card tight"
        : "settings-card tight dimmed"
    }
  >
    <div className="settings-row">
      <div className="settings-row-text">
        <strong>文件夹筛选</strong>
        <span>
          {googlePrefs?.sync_mode !== "recent"
            ? "切换到「最近文档」后可用"
            : (googlePrefs?.folder_ids?.length ?? 0) > 0
              ? `${googlePrefs!.folder_ids.length} 个 · 仅直接子文档`
              : "可选 · 不填则不限文件夹"}
        </span>
      </div>
      {(googlePrefs?.folder_ids?.length ?? 0) > 0 &&
        googlePrefs?.sync_mode === "recent" && (
          <button
            type="button"
            className="btn ghost sm"
            disabled={googleBusy}
            onClick={() => void handleClearFolderFilter()}
          >
            清除
          </button>
        )}
    </div>
    {(googlePrefs?.folder_ids?.length ?? 0) > 0 && (
      <ul className="folder-id-list">
        {googlePrefs!.folder_ids.map((id) => (
          <li key={id}>
            <code className="mono small">{id}</code>
            <button
              type="button"
              className="btn ghost sm"
              disabled={
                googleBusy ||
                googlePrefs?.sync_mode !== "recent"
              }
              onClick={() => void handleRemoveOneFolder(id)}
            >
              移除
            </button>
          </li>
        ))}
      </ul>
    )}
    <textarea
      className="folder-filter-input"
      rows={3}
      value={folderFilterText}
      onChange={(e) => setFolderFilterText(e.target.value)}
      placeholder="粘贴 Drive 文件夹链接或 ID（可多行）"
      disabled={
        googleBusy || googlePrefs?.sync_mode !== "recent"
      }
    />
    <div className="form-actions tight">
      <button
        type="button"
        className="btn primary sm"
        disabled={
          googleBusy ||
          !folderFilterText.trim() ||
          googlePrefs?.sync_mode !== "recent"
        }
        onClick={() => void handleSaveFolderFilter()}
      >
        保存筛选
      </button>
    </div>
  </div>

  <div className="settings-card tight">
    <div className="settings-row" style={{ flexDirection: "column", alignItems: "stretch" }}>
      <div className="settings-row-text">
        <strong>Notion 集成 {notionConnected ? "· Token 已保存" : ""}</strong>
        <span>
          {"在 notion.so/my-integrations 创建 Integration，并在目标数据库「连接」该 Integration。"}
          {"数据库 ID 取自其 URL 中的 32 位十六进制字符串（?v= 之前的那段）。"}
          {"添加后点击来源页的「同步」拉取页面正文。"}
        </span>
      </div>
      <input
        type="password"
        className="folder-filter-input"
        style={{ marginTop: "8px" }}
        placeholder="Integration Token（secret_ 开头）"
        value={notionToken}
        spellCheck={false}
        autoComplete="off"
        onChange={(e) => setNotionToken(e.target.value)}
      />
      <input
        type="text"
        className="folder-filter-input"
        style={{ marginTop: "6px" }}
        placeholder="数据库 ID 或数据库链接"
        value={notionDatabaseId}
        spellCheck={false}
        onChange={(e) => setNotionDatabaseId(e.target.value)}
      />
      <div className="form-actions tight">
        <button
          type="button"
          className="btn primary sm"
          disabled={
            notionBusy ||
            !notionToken.trim() ||
            !notionDatabaseId.trim()
          }
          onClick={() => void handleAddNotionSource()}
        >
          {notionBusy ? "验证中…" : "添加 Notion 数据库"}
        </button>
      </div>
    </div>
  </div>
</div>
  );
}
