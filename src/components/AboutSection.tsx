import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as api from "../lib/api";
import type { UpdateInfo } from "../types";
import "../styles/AboutSection.css";

export function AboutSection() {
  const [version, setVersion] = useState<string>("");
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<UpdateInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .getAppVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

  async function handleCheckUpdate() {
    try {
      setChecking(true);
      setError(null);
      setResult(await api.checkAppUpdate());
    } catch (err) {
      setError(String(err));
    } finally {
      setChecking(false);
    }
  }

  return (
  <div className="settings-detail-body">
    <header className="settings-detail-head">
      <div>
        <h2>关于 SearchDoc</h2>
        <p>本地优先的跨源正文搜索</p>
      </div>
      <span className="pill">v{version || "…"}</span>
    </header>

    <div className="settings-card tight">
      <div className="settings-row">
        <div className="settings-row-text">
          <strong>它是什么</strong>
          <span>
            本地优先的跨源正文检索：统一搜本地文件、Google Docs 与 Notion
            的内容，给片段、预览、打开原文。
          </span>
        </div>
      </div>
      <div className="settings-divider" />
      <div className="settings-row">
        <div className="settings-row-text">
          <strong>它不是什么</strong>
          <span>
            不是 Everything（文件名）、不是笔记编辑器、不是
            AI 问答。只把「这句话写在哪」做快、做准。
          </span>
        </div>
      </div>
      <div className="settings-divider" />
      <div className="settings-row">
        <div className="settings-row-text">
          <strong>搜索语法</strong>
          <span>
            空格 = 且 · OR / | = 或 · NOT / -词 = 排除 ·
            「更多」里可限标题/正文与排序
          </span>
        </div>
      </div>
      <div className="settings-divider" />
      <div className="settings-row">
        <div className="settings-row-text">
          <strong>检查更新</strong>
          <span>
            {result === null
              ? "对比 GitHub Releases 上的最新版本"
              : result.disabled
                ? "尚未配置发布仓库，检查已停用"
                : result.updateAvailable
                  ? `发现新版本 v${result.latest}（当前 v${result.current}）`
                  : `已是最新版本（v${result.current}）`}
            {result?.notes ? ` · ${result.notes.slice(0, 120)}` : ""}
          </span>
          {error && (
            <span style={{ color: "#c0392b" }}>{error}</span>
          )}
        </div>
        <button
          type="button"
          className="btn ghost sm"
          disabled={checking}
          onClick={() => void handleCheckUpdate()}
        >
          {checking ? "检查中…" : "检查更新"}
        </button>
      </div>
      {result?.updateAvailable && result.releaseUrl && (
        <div className="settings-row">
          <div className="settings-row-text">
            <span>前往发布页下载新版本安装包。</span>
          </div>
          <button
            type="button"
            className="btn primary sm"
            onClick={() => void openUrl(result.releaseUrl as string)}
          >
            打开发布页
          </button>
        </div>
      )}
    </div>
  </div>
  );
}
