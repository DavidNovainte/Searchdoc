import "../styles/SourceList.css";
import type { GoogleAuthStatus, GooglePrefsStatus, SourceInfo } from "../types";
import type { SettingsSection } from "../lib/format";
import { formatTime, sourceLabel } from "../lib/format";
import { IconCloud, IconFolder, IconSync } from "../icons";

export interface SourceListProps {
  sources: SourceInfo[];
  googleAuth: GoogleAuthStatus | null;
  googlePrefs: GooglePrefsStatus | null;
  syncing: boolean;
  googleBusy: boolean;
  busy: boolean;
  handleAddFolder: () => void;
  openSettings: (section: SettingsSection) => void;
  handleSyncOne: (id: string) => void;
  handleToggle: (source: SourceInfo) => void;
  handleRemove: (id: string) => void;
}

export function SourceList(props: SourceListProps) {
  const {
    sources, googleAuth, googlePrefs, syncing, googleBusy, busy,
    handleAddFolder, openSettings, handleSyncOne, handleToggle, handleRemove,
  } = props;
  return sources.length === 0 ? (
  <div className="empty">
    <h3>还没有来源</h3>
    <p>添加本地文件夹，或到设置连接 Google。</p>
    <div className="empty-actions">
      <button
        className="btn primary sm"
        onClick={handleAddFolder}
        disabled={busy}
      >
        添加本地文件夹
      </button>
      <button
        className="btn ghost sm"
        onClick={() => {
          openSettings("google");
        }}
      >
        连接 Google
      </button>
    </div>
  </div>
) : (
  <div className="source-list">
    {sources.map((source) => (
      <article
        key={source.id}
        className={`source-card${source.enabled ? "" : " disabled"}`}
      >
        <div
          className={`source-avatar ${source.kind}`}
          aria-hidden
        >
          {source.kind === "google_docs" ? (
            <IconCloud size={18} />
          ) : (
            <IconFolder size={18} />
          )}
        </div>
        <div className="source-main">
          <div className="source-title-row">
            <span className={`badge ${source.kind}`}>
              {sourceLabel(source.kind)}
            </span>
            <h3>{source.name}</h3>
            {!source.enabled && (
              <span className="pill">已禁用</span>
            )}
          </div>
          <p
            className="source-path"
            title={source.root_path ?? undefined}
          >
            {source.root_path ??
              (googleAuth?.connected
                ? googlePrefs?.sync_mode === "recent"
                  ? "最近文档浅同步"
                  : "仅观察列表"
                : "未连接 Google")}
          </p>
          <div className="source-meta">
            <span>{source.doc_count} 篇</span>
            <span>{formatTime(source.last_sync_at)}</span>
          </div>
        </div>
        <div className="source-actions">
          <button
            type="button"
            className="btn ghost sm with-ico"
            disabled={syncing || googleBusy}
            onClick={() => handleSyncOne(source.id)}
          >
            <IconSync size={13} />
            同步
          </button>
          <button
            type="button"
            className="btn ghost sm"
            onClick={() => handleToggle(source)}
          >
            {source.enabled ? "禁用" : "启用"}
          </button>
          {source.kind === "local" && (
            <button
              type="button"
              className="btn danger sm"
              onClick={() => handleRemove(source.id)}
            >
              移除
            </button>
          )}
        </div>
      </article>
    ))}
  </div>
  );
}
