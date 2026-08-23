import "../styles/PreviewPanel.css";
import type { RefObject } from "react";
import type { SearchHit, SearchMode } from "../types";
import { formatTime, relativeTime, shortPath, sourceLabel } from "../lib/format";
import { highlightPreview, highlightSmartSnippet } from "../lib/highlight";
import { IconCloud, IconCopy, IconExternal, IconFileText, IconFolder, IconLink } from "../icons";

export interface PreviewPanelProps {
  selected: SearchHit | null;
  previewLoading: boolean;
  previewBody: string;
  previewError: string | null;
  activeMatch: number;
  matchCount: number;
  query: string;
  searchMode: SearchMode;
  goPrevMatch: () => void;
  goNextMatch: () => void;
  handleOpen: (hit: SearchHit) => void;
  handleReveal: (hit: SearchHit) => void;
  handleCopyPath: (uri: string) => void;
  previewScrollRef: RefObject<HTMLDivElement | null>;
}

export function PreviewPanel(props: PreviewPanelProps) {
  const {
    selected, previewLoading, previewBody, previewError, activeMatch, matchCount,
    query, searchMode, goPrevMatch, goNextMatch,
    handleOpen, handleReveal, handleCopyPath, previewScrollRef,
  } = props;
  return (
<aside className="panel detail-panel">
  {selected ? (
    <>
      <div className="detail-header">
        <div className="detail-header-top">
          <span
            className={`result-kind lg ${selected.source_kind}`}
            title={sourceLabel(selected.source_kind)}
          >
            {selected.source_kind === "google_docs" ? (
              <IconCloud size={16} />
            ) : (
              <IconFolder size={16} />
            )}
          </span>
          <div className="detail-header-tags">
            <span className={`badge ${selected.source_kind}`}>
              {sourceLabel(selected.source_kind)}
            </span>
            {(selected.depth ?? 0) > 0 && (
              <span className="badge deep">
                <IconLink size={11} />
                外链
              </span>
            )}
            {selected.mtime && (
              <span className="detail-time">
                {relativeTime(selected.mtime) ||
                  formatTime(selected.mtime)}
              </span>
            )}
          </div>
          <div className="detail-icon-actions">
            <button
              type="button"
              className="icon-btn"
              title={
                selected.source_kind === "local"
                  ? "在文件夹中显示"
                  : "在浏览器打开"
              }
              onClick={() => void handleReveal(selected)}
            >
              {selected.source_kind === "local" ? (
                <IconFolder size={15} />
              ) : (
                <IconExternal size={15} />
              )}
            </button>
            <button
              type="button"
              className="icon-btn"
              title="复制路径/链接"
              onClick={() => void handleCopyPath(selected.uri)}
            >
              <IconCopy size={15} />
            </button>
            <button
              type="button"
              className="icon-btn primary"
              title="打开原文"
              onClick={() => handleOpen(selected)}
            >
              <IconExternal size={15} />
            </button>
          </div>
        </div>
        <h2 title={selected.title}>{selected.title}</h2>
        <p className="detail-uri" title={selected.uri}>
          {shortPath(selected.uri)}
        </p>
        {(selected.depth ?? 0) > 0 && selected.via && (
          <p className="via-line">
            <IconLink size={12} /> 来自 {selected.via}
          </p>
        )}
      </div>

      <div className="detail-body">
        <div className="preview-section">
          <div className="preview-head">
            <h5>全文预览</h5>
            {previewLoading && <span className="spinner" />}
            {!previewLoading && previewBody && query.trim() && (
              <div className="match-nav">
                <span className="match-count">
                  {matchCount > 0
                    ? `${activeMatch + 1} / ${matchCount}`
                    : "0 处"}
                </span>
                <button
                  type="button"
                  className="btn ghost sm match-btn"
                  disabled={matchCount <= 0}
                  onClick={goPrevMatch}
                  title="上一处"
                >
                  上一个
                </button>
                <button
                  type="button"
                  className="btn ghost sm match-btn"
                  disabled={matchCount <= 0}
                  onClick={goNextMatch}
                  title="下一处"
                >
                  下一个
                </button>
              </div>
            )}
          </div>
          {!previewLoading && selected.snippet && (
            <div className="snippet-inline" title="当前命中上下文">
              {highlightSmartSnippet(
                selected.snippet,
                query,
                searchMode,
              )}
            </div>
          )}
          <div
            className="preview-card"
            id="doc-preview"
            ref={previewScrollRef}
          >
            {previewLoading ? (
              <p className="preview-muted">正在加载正文…</p>
            ) : previewError ? (
              <p className="preview-muted">{previewError}</p>
            ) : previewBody ? (
              <div className="preview-text">
                {highlightPreview(
                  previewBody,
                  query,
                  searchMode,
                  activeMatch,
                )}
              </div>
            ) : (
              <p className="preview-muted">无正文</p>
            )}
          </div>
        </div>
      </div>

      <div className="detail-actions">
        <button
          className="btn primary block"
          onClick={() => handleOpen(selected)}
        >
          <IconExternal size={16} />
          打开原文
        </button>
      </div>
    </>
  ) : (
    <div className="detail-empty">
      <div className="detail-empty-card">
        <div className="detail-empty-ico">
          <IconFileText size={26} />
        </div>
        <h3>选择一条结果</h3>
        <p>
          在左侧搜索并点选文档，这里会预览全文并高亮关键词。
        </p>
        <ul className="detail-empty-tips">
          <li>双击结果可直接打开原文</li>
          <li>↑↓ 切换 · Enter 打开</li>
        </ul>
      </div>
    </div>
  )}
</aside>
  );
}
