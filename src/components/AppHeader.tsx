import "../styles/AppHeader.css";
import type { SettingsSection, Tab } from "../lib/format";
import { tabSubtitle, tabTitle } from "../lib/format";
import { IconChevronLeft } from "../icons";

export interface AppHeaderProps {
  tab: Tab;
  inSettings: boolean;
  settingsSection: SettingsSection;
  leaveSettings: () => void;
  success: string | null;
  setSuccess: (value: string | null) => void;
  error: string | null;
  setError: (value: string | null) => void;
}

export function AppHeader(props: AppHeaderProps) {
  const {
    tab, inSettings, settingsSection, leaveSettings, success, setSuccess, error, setError,
  } = props;
  return (
    <>
        {tab !== "search" && (
          <header className="main-header">
            <div className="main-header-lead">
              {inSettings && (
                <button
                  type="button"
                  className="back-btn"
                  onClick={leaveSettings}
                  title="返回"
                >
                  <IconChevronLeft size={18} />
                  <span>返回</span>
                </button>
              )}
              <div className="main-header-text">
                <h1>{tabTitle(tab)}</h1>
                <p>{tabSubtitle(tab, settingsSection)}</p>
              </div>
            </div>
            <div className="header-actions">
              {success && (
                <div
                  className="header-toast success"
                  onClick={() => setSuccess(null)}
                  title="点击关闭"
                >
                  <span className="header-toast-text">{success}</span>
                  <button
                    type="button"
                    className="header-toast-close"
                    onClick={(e) => {
                      e.stopPropagation();
                      setSuccess(null);
                    }}
                    aria-label="关闭提示"
                  >
                    ×
                  </button>
                </div>
              )}
            </div>
          </header>
        )}

        {tab === "search" && success && (
          <div className="search-toast-bar">
            <div
              className="header-toast success"
              onClick={() => setSuccess(null)}
              title="点击关闭"
            >
              <span className="header-toast-text">{success}</span>
              <button
                type="button"
                className="header-toast-close"
                onClick={(e) => {
                  e.stopPropagation();
                  setSuccess(null);
                }}
                aria-label="关闭提示"
              >
                ×
              </button>
            </div>
          </div>
        )}

        {error && (
          <div className="banner error" onClick={() => setError(null)}>
            <span>{error}</span>
            <span>关闭</span>
          </div>
        )}
    </>
  );
}
