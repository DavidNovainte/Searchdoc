import "../styles/GoogleSection.css";
import type { GoogleAuthStatus } from "../types";

export interface GoogleSectionProps {
  googleAuth: GoogleAuthStatus | null;
  clientId: string;
  setClientId: (value: string) => void;
  clientSecret: string;
  setClientSecret: (value: string) => void;
  googleBusy: boolean;
  handleConnectGoogle: () => void;
  handleDisconnectGoogle: () => void;
  handleSaveGoogleConfig: () => void;
}

export function GoogleSection(props: GoogleSectionProps) {
  const {
    googleAuth, clientId, setClientId, clientSecret, setClientSecret, googleBusy,
    handleConnectGoogle, handleDisconnectGoogle, handleSaveGoogleConfig,
  } = props;
  return (
<div className="settings-detail-body">
  <header className="settings-detail-head">
    <div>
      <h2>Google 账号</h2>
      <p>桌面 OAuth · 连接后可同步 Docs</p>
    </div>
    <span
      className={`conn-pill ${googleAuth?.connected ? "on" : "off"}`}
    >
      {googleAuth?.connected
        ? "已连接"
        : googleAuth?.configured
          ? "已配置"
          : "未配置"}
    </span>
  </header>

  <div className="settings-card tight">
    <div className="settings-row">
      <div className="settings-row-text">
        <strong>连接状态</strong>
        <span>
          {googleAuth?.connected
            ? "可同步 Docs / 观察列表"
            : "填写凭据后连接"}
        </span>
      </div>
      <div className="settings-row-actions">
        <button
          type="button"
          className="btn primary sm"
          disabled={
            googleBusy ||
            !(
              googleAuth?.configured ||
              (clientId && clientSecret)
            )
          }
          onClick={() => void handleConnectGoogle()}
        >
          {googleBusy
            ? "…"
            : googleAuth?.connected
              ? "重连"
              : "连接"}
        </button>
        {googleAuth?.connected && (
          <button
            type="button"
            className="btn danger sm"
            disabled={googleBusy}
            onClick={() => void handleDisconnectGoogle()}
          >
            断开
          </button>
        )}
      </div>
    </div>
  </div>

  <div className="settings-card tight">
    <div className="settings-row">
      <div className="settings-row-text">
        <strong>OAuth 凭据</strong>
        <span>Google Cloud 桌面应用客户端</span>
      </div>
    </div>
    <div className="form-grid compact">
      <label>
        <span>Client ID</span>
        <input
          value={clientId}
          onChange={(e) => setClientId(e.target.value)}
          placeholder="….apps.googleusercontent.com"
          autoComplete="off"
        />
      </label>
      <label>
        <span>Client Secret</span>
        <input
          type="password"
          value={clientSecret}
          onChange={(e) => setClientSecret(e.target.value)}
          placeholder={
            googleAuth?.configured
              ? "已保存 · 可覆盖"
              : "client secret"
          }
          autoComplete="off"
        />
      </label>
    </div>
    <div className="form-actions tight">
      <button
        type="button"
        className="btn ghost sm"
        disabled={
          googleBusy ||
          !clientId.trim() ||
          !clientSecret.trim()
        }
        onClick={() => void handleSaveGoogleConfig()}
      >
        保存凭据
      </button>
      <span className="settings-hint">docs/GOOGLE_SETUP.md</span>
    </div>
  </div>
</div>
  );
}
