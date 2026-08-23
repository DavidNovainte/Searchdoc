import "../styles/Sidebar.css";
import { useEffect, useRef, useState } from "react";
import type { GoogleAuthStatus, IndexStats } from "../types";
import type { SettingsSection, Tab } from "../lib/format";
import {
  IconChevronLeft,
  IconChevronRight,
  IconClose,
  IconCloud,
  IconSearch,
  IconSources,
  IconSync,
  IconUser,
} from "../icons";

export interface SidebarProps {
  tab: Tab;
  setTab: (tab: Tab) => void;
  collapsed: boolean;
  onToggleSidebar: () => void;
  indexBusy: boolean;
  busy: boolean;
  status: string;
  syncPhase: string | null;
  syncing: boolean;
  googleAuth: GoogleAuthStatus | null;
  bootstrapping: boolean;
  stats: IndexStats | null;
  handleCancelSync: () => void;
  handleSyncAll: () => void;
  openSettings: (section: SettingsSection) => void;
}

export function Sidebar(props: SidebarProps) {
  const {
    tab, setTab, collapsed, onToggleSidebar, indexBusy, busy, status, syncPhase,
    syncing, googleAuth, bootstrapping, stats, handleCancelSync, handleSyncAll, openSettings,
  } = props;
  const [accountMenuOpen, setAccountMenuOpen] = useState(false);
  const accountMenuRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!accountMenuOpen) return;
    function onDocDown(e: MouseEvent) {
      const el = accountMenuRef.current;
      if (el && !el.contains(e.target as Node)) setAccountMenuOpen(false);
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setAccountMenuOpen(false);
    }
    document.addEventListener("mousedown", onDocDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDocDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [accountMenuOpen]);

  return (
      <aside className={`sidebar${collapsed ? " collapsed" : ""}`}>
        <div className="brand">
          <div className="brand-mark" title="SearchDoc">
            SD
          </div>
          {!collapsed && (
            <div className="brand-text">
              <div className="brand-title">SearchDoc</div>
              <div className="brand-sub">本地 · Google 正文检索</div>
            </div>
          )}
        </div>

        <button
          type="button"
          className="sidebar-toggle"
          onClick={() => { setAccountMenuOpen(false); onToggleSidebar(); }}
          title={collapsed ? "展开侧栏（图标+文字）" : "收起侧栏（仅图标）"}
          aria-label={collapsed ? "展开侧栏" : "收起侧栏"}
          aria-pressed={collapsed}
        >
          <span className="sidebar-toggle-ico" aria-hidden>
            {collapsed ? (
              <IconChevronRight size={16} />
            ) : (
              <IconChevronLeft size={16} />
            )}
          </span>
          {!collapsed && (
            <span className="sidebar-toggle-label">收起侧栏</span>
          )}
        </button>

        <nav className="nav">
          <button
            className={tab === "search" ? "nav-btn active" : "nav-btn"}
            onClick={() => setTab("search")}
            title="搜索"
          >
            <span className="nav-ico">
              <IconSearch size={18} />
            </span>
            {!collapsed && <span className="nav-label">搜索</span>}
          </button>
          <button
            className={tab === "sources" ? "nav-btn active" : "nav-btn"}
            onClick={() => setTab("sources")}
            title="来源"
          >
            <span className="nav-ico">
              <IconSources size={18} />
            </span>
            {!collapsed && <span className="nav-label">来源</span>}
          </button>
        </nav>

        <div className="sidebar-foot account-foot">
          {!collapsed &&
            (indexBusy ||
              (status && status !== "准备就绪" && status !== "同步完成")) && (
              <div className={`status-line ${indexBusy ? "busy" : ""}`}>
                {syncPhase || status}
              </div>
            )}
          {indexBusy && (
            <div
              className="sync-progress"
              aria-hidden
              title={syncPhase || status || "同步中"}
            >
              <div className="sync-progress-bar" />
            </div>
          )}
          {syncing && (
            <button
              type="button"
              className={
                collapsed
                  ? "btn danger sm sidebar-sync icon-only"
                  : "btn danger sm sidebar-sync"
              }
              onClick={() => void handleCancelSync()}
              title="取消同步"
            >
              {collapsed ? (
                <IconClose size={16} />
              ) : (
                "取消同步"
              )}
            </button>
          )}

          <div
            className={`account-menu-wrap${accountMenuOpen ? " open" : ""}`}
            ref={accountMenuRef}
          >
            {accountMenuOpen && (
              <div className="account-menu" role="menu">
                <div className="account-menu-head">
                  <span className={`dot ${googleAuth?.connected ? "on" : ""}`} />
                  <div>
                    <strong>
                      {googleAuth?.connected ? "Google 已连接" : "本机模式"}
                    </strong>
                    <p>
                      {bootstrapping
                        ? "…"
                        : `${stats?.document_count ?? 0} 篇 · 本地 ${stats?.local_doc_count ?? 0} · G ${stats?.google_doc_count ?? 0}`}
                    </p>
                  </div>
                </div>
                <div className="account-menu-sep" />
                <button
                  type="button"
                  role="menuitem"
                  className="account-menu-item"
                  onClick={() => openSettings("google")}
                >
                  <IconCloud size={16} />
                  <span>
                    {googleAuth?.connected ? "Google 账号" : "连接 Google…"}
                  </span>
                </button>
                <button
                  type="button"
                  role="menuitem"
                  className="account-menu-item"
                  onClick={() => openSettings("sync")}
                >
                  <IconSync size={16} />
                  <span>同步范围</span>
                </button>
                <button
                  type="button"
                  role="menuitem"
                  className="account-menu-item"
                  disabled={busy || syncing}
                  onClick={() => {
                    setAccountMenuOpen(false);
                    void handleSyncAll();
                  }}
                >
                  <IconSync size={16} />
                  <span>同步全部</span>
                </button>
                <div className="account-menu-sep" />
                <button
                  type="button"
                  role="menuitem"
                  className="account-menu-item"
                  onClick={() => openSettings("system")}
                >
                  <IconUser size={16} />
                  <span>设置</span>
                </button>
                <button
                  type="button"
                  role="menuitem"
                  className="account-menu-item"
                  onClick={() => openSettings("about")}
                >
                  <span className="account-menu-mark">SD</span>
                  <span>关于 SearchDoc</span>
                </button>
              </div>
            )}

            <button
              type="button"
              className={`account-chip${googleAuth?.connected ? " on" : ""}${
                collapsed ? " compact" : ""
              }`}
              onClick={() => setAccountMenuOpen((v) => !v)}
              aria-haspopup="menu"
              aria-expanded={accountMenuOpen}
              title={
                googleAuth?.connected
                  ? "Google 已连接 · 账户与设置"
                  : "本机模式 · 账户与设置"
              }
            >
              {collapsed ? (
                <span className="account-chip-avatar">
                  <IconUser size={16} />
                </span>
              ) : (
                <>
                  <span className="dot" />
                  <span className="account-chip-text">
                    <strong>
                      {googleAuth?.connected ? "Google" : "本机"}
                    </strong>
                    <span>
                      {bootstrapping
                        ? "…"
                        : googleAuth?.connected
                          ? `${stats?.document_count ?? 0} 篇可搜`
                          : "未连接云端"}
                    </span>
                  </span>
                  <span className="account-chip-caret" aria-hidden>
                    {accountMenuOpen ? (
                      <IconChevronRight
                        size={14}
                        style={{ transform: "rotate(-90deg)" }}
                      />
                    ) : (
                      <IconChevronRight
                        size={14}
                        style={{ transform: "rotate(90deg)" }}
                      />
                    )}
                  </span>
                </>
              )}
            </button>
          </div>
        </div>
      </aside>

  );
}
