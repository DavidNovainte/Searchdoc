import "../styles/SystemSection.css";

export interface SystemSectionProps {
  autostartOn: boolean;
  autostartBusy: boolean;
  handleToggleAutostart: () => void;
  optimizeBusy: boolean;
  handleOptimizeIndex: () => void;
  shortcutInput: string;
  setShortcutInput: (value: string) => void;
  shortcutBusy: boolean;
  handleApplyShortcut: () => void;
}

export function SystemSection(props: SystemSectionProps) {
  const {
    autostartOn,
    autostartBusy,
    handleToggleAutostart,
    optimizeBusy,
    handleOptimizeIndex,
    shortcutInput,
    setShortcutInput,
    shortcutBusy,
    handleApplyShortcut,
  } = props;
  return (
<div className="settings-detail-body">
  <header className="settings-detail-head">
    <div>
      <h2>系统</h2>
      <p>启动与常驻</p>
    </div>
  </header>
  <div className="settings-card tight">
    <div className="settings-row">
      <div className="settings-row-text">
        <strong>开机自启</strong>
        <span>登录后托盘静默 · 全局快捷键随时唤起</span>
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={autostartOn}
        className={autostartOn ? "toggle on" : "toggle"}
        disabled={autostartBusy}
        onClick={() => void handleToggleAutostart()}
      >
        <span className="toggle-knob" />
      </button>
    </div>
    <div className="settings-row">
      <div className="settings-row-text">
        <strong>全局快捷键</strong>
        <span>唤起主窗口 · 示例 Ctrl+Shift+Space、Alt+F9、Ctrl+Alt+S</span>
        <input
          type="text"
          className="folder-filter-input"
          style={{ width: "200px", marginTop: "6px", display: "block" }}
          placeholder="输入快捷键，如 Ctrl+Shift+Space"
          value={shortcutInput}
          spellCheck={false}
          onChange={(e) => setShortcutInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !shortcutBusy && shortcutInput.trim()) {
              e.preventDefault();
              handleApplyShortcut();
            }
          }}
        />
      </div>
      <button
        type="button"
        className="btn ghost sm"
        disabled={shortcutBusy || !shortcutInput.trim()}
        onClick={() => void handleApplyShortcut()}
      >
        {shortcutBusy ? "应用中…" : "应用"}
      </button>
    </div>
    <div className="settings-row">
      <div className="settings-row-text">
        <strong>优化索引</strong>
        <span>整理全文索引碎片并回收空间 · 大批量同步后建议执行</span>
      </div>
      <button
        type="button"
        className="btn ghost sm"
        disabled={optimizeBusy}
        onClick={() => void handleOptimizeIndex()}
      >
        {optimizeBusy ? "优化中…" : "立即优化"}
      </button>
    </div>
  </div>
</div>
  );
}
