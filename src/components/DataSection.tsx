import "../styles/DataSection.css";
import type { GoogleAuthStatus, IndexStats } from "../types";

export interface DataSectionProps {
  stats: IndexStats | null;
  googleAuth: GoogleAuthStatus | null;
  backupBusy: boolean;
  handleBackupIndex: () => void;
  restoreBusy: boolean;
  handleRestoreIndex: () => void;
}

export function DataSection(props: DataSectionProps) {
  const { stats, googleAuth, backupBusy, handleBackupIndex, restoreBusy, handleRestoreIndex } = props;
  return (
<div className="settings-detail-body">
  <header className="settings-detail-head">
    <div>
      <h2>本机数据</h2>
      <p>全部保存在本机，不上传</p>
    </div>
  </header>
  <div className="settings-card tight">
    <div className="path-list">
      <div className="path-row">
        <span>索引库</span>
        <code
          className="mono small"
          title={stats?.db_path ?? ""}
        >
          {stats?.db_path ?? "—"}
        </code>
      </div>
      <div className="path-row">
        <span>OAuth</span>
        <code
          className="mono small"
          title={googleAuth?.config_path ?? ""}
        >
          {googleAuth?.config_path ?? "—"}
        </code>
      </div>
      <div className="path-row">
        <span>Token</span>
        <code
          className="mono small"
          title={googleAuth?.token_path ?? ""}
        >
          {googleAuth?.token_path ?? "—"}
        </code>
      </div>
    </div>
  </div>
  <div className="settings-card tight">
    <div className="settings-row">
      <div className="settings-row-text">
        <strong>备份索引</strong>
        <span>创建可独立打开的一致性 SQLite 备份</span>
      </div>
      <button
        type="button"
        className="btn ghost sm"
        disabled={backupBusy}
        onClick={() => void handleBackupIndex()}
      >
        {backupBusy ? "备份中…" : "立即备份"}
      </button>
    </div>
    <div className="settings-row">
      <div className="settings-row-text">
        <strong>恢复索引</strong>
        <span>验证备份后，在下次启动时安全替换当前索引</span>
      </div>
      <button
        type="button"
        className="btn ghost sm"
        disabled={restoreBusy}
        onClick={() => void handleRestoreIndex()}
      >
        {restoreBusy ? "验证中…" : "选择备份"}
      </button>
    </div>
  </div>
</div>
  );
}
