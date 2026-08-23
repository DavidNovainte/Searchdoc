import "../styles/SettingsNav.css";
import type { Dispatch, SetStateAction } from "react";
import type { GoogleAuthStatus, GooglePrefsStatus } from "../types";
import type { SettingsSection } from "../lib/format";

const NAV_ITEMS: {
  id: SettingsSection;
  label: string;
  hint: string;
}[] = [
  { id: "google", label: "Google 账号", hint: "OAuth 连接" },
  { id: "sync", label: "同步范围", hint: "观察列表 / 最近" },
  { id: "system", label: "系统", hint: "开机自启" },
  { id: "data", label: "本机数据", hint: "索引与路径" },
  { id: "about", label: "关于", hint: "版本与快捷键" },
];

export interface SettingsNavProps {
  settingsSection: SettingsSection;
  setSettingsSection: Dispatch<SetStateAction<SettingsSection>>;
  googleAuth: GoogleAuthStatus | null;
  googlePrefs: GooglePrefsStatus | null;
  autostartOn: boolean;
}

export function SettingsNav(props: SettingsNavProps) {
  const { settingsSection, setSettingsSection, googleAuth, googlePrefs, autostartOn } = props;
  return (
<nav className="settings-nav" aria-label="设置分类">
  {NAV_ITEMS.map((item) => {
    const active = settingsSection === item.id;
    const badge =
      item.id === "google"
        ? googleAuth?.connected
          ? "已连接"
          : googleAuth?.configured
            ? "已配置"
            : "未配置"
        : item.id === "sync"
          ? googlePrefs?.sync_mode === "recent"
            ? "最近"
            : "观察"
          : item.id === "system"
            ? autostartOn
              ? "开"
              : "关"
            : null;
    const badgeOn =
      (item.id === "google" && !!googleAuth?.connected) ||
      (item.id === "system" && autostartOn);
    return (
      <button
        key={item.id}
        type="button"
        className={active ? "settings-nav-item active" : "settings-nav-item"}
        onClick={() => setSettingsSection(item.id)}
      >
        <span className="settings-nav-main">
          <strong>{item.label}</strong>
          <span>{item.hint}</span>
        </span>
        {badge && (
          <span
            className={
              badgeOn ? "settings-nav-badge on" : "settings-nav-badge"
            }
          >
            {badge}
          </span>
        )}
      </button>
    );
  })}
</nav>
  );
}
