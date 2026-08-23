import "../styles/ResultList.css";
import { memo, useCallback, useRef } from "react";
import type { Dispatch, RefObject, SetStateAction } from "react";
import type {
  SearchHit,
  SearchMode,
  SearchScope,
  SourceFilter,
} from "../types";
import type { Tab } from "../lib/format";
import { relativeTime, shortPath } from "../lib/format";
import { highlightSmartSnippet } from "../lib/highlight";
import { IconCloud, IconFolder, IconLink } from "../icons";

interface ResultItemProps {
  hit: SearchHit;
  selected: boolean;
  query: string;
  searchMode: SearchMode;
  onSelect: (id: string) => void;
  onOpen: (hit: SearchHit) => void;
  registerRef: (id: string, el: HTMLButtonElement | null) => void;
}

/** Memoized row: selection changes re-render only the two affected rows
 * instead of re-highlighting every snippet in the list. */
const ResultItem = memo(function ResultItem({
  hit, selected, query, searchMode, onSelect, onOpen, registerRef,
}: ResultItemProps) {
  return (
    <button
      ref={(el) => {
        registerRef(hit.id, el);
      }}
      className={selected ? "result-item active" : "result-item"}
      onClick={() => onSelect(hit.id)}
      onDoubleClick={() => onOpen(hit)}
    >
      <div className="result-top">
        <span
          className={`result-kind ${hit.source_kind}`}
          title={
            hit.source_kind === "google_docs"
              ? "Google Docs"
              : hit.source_kind === "notion"
                ? "Notion"
                : "本地"
          }
        >
          {hit.source_kind === "google_docs" || hit.source_kind === "notion" ? (
            <IconCloud size={14} />
          ) : (
            <IconFolder size={14} />
          )}
        </span>
        <h4>{hit.title}</h4>
        {relativeTime(hit.mtime) ? (
          <time className="result-time">
            {relativeTime(hit.mtime)}
          </time>
        ) : null}
      </div>
      <p className="snippet">
        {highlightSmartSnippet(
          hit.snippet,
          query,
          searchMode,
        )}
      </p>
      <div className="result-meta" title={hit.uri}>
        <span className={`badge ${hit.source_kind}`}>
          {hit.source_kind === "google_docs"
            ? "Google"
            : hit.source_kind === "notion"
              ? "Notion"
              : "本地"}
        </span>
        {(hit.depth ?? 0) > 0 && (
          <span
            className="badge deep"
            title={
              hit.via ? `来自：${hit.via}` : "外链命中"
            }
          >
            <IconLink size={11} />
            外链
          </span>
        )}
        <span className="result-path">
          {shortPath(hit.uri)}
        </span>
      </div>
    </button>
  );
});

export interface ResultListProps {
  hits: SearchHit[];
  setHits: Dispatch<SetStateAction<SearchHit[]>>;
  selectedId: string | null;
  setSelectedId: Dispatch<SetStateAction<string | null>>;
  query: string;
  setQuery: Dispatch<SetStateAction<string>>;
  resultItemRefs: RefObject<Record<string, HTMLButtonElement | null>>;
  setTab: (tab: Tab) => void;
  handleOpen: (hit: SearchHit) => void;
  handleAddFolder: () => void;
  handleSyncAll: () => void;
  indexBusy: boolean;
  hasIndexedDocs: boolean;
  searchMode: SearchMode;
  setSearchMode: Dispatch<SetStateAction<SearchMode>>;
  sourceFilter: SourceFilter;
  setSourceFilter: Dispatch<SetStateAction<SourceFilter>>;
  searchScope: SearchScope;
  setSearchScope: Dispatch<SetStateAction<SearchScope>>;
  setShowSearchMore: Dispatch<SetStateAction<boolean>>;
  searchInputRef: RefObject<HTMLInputElement | null>;
  hasMore: boolean;
  loading: boolean;
  loadMore: () => void;
}

export function ResultList(props: ResultListProps) {
  const {
    hits, setHits, selectedId, setSelectedId, query, setQuery,
    resultItemRefs, setTab, handleOpen, handleAddFolder, handleSyncAll,
    indexBusy, hasIndexedDocs,
    searchMode, setSearchMode, sourceFilter, setSourceFilter,
    searchScope, setSearchScope, setShowSearchMore, searchInputRef,
    hasMore, loading, loadMore,
  } = props;

  // Stable callbacks keep memoized rows from re-rendering needlessly.
  const handleSelect = useCallback(
    (id: string) => setSelectedId(id),
    [setSelectedId],
  );
  const openRef = useRef(handleOpen);
  openRef.current = handleOpen;
  const handleOpenStable = useCallback(
    (hit: SearchHit) => openRef.current(hit),
    [],
  );
  const registerRef = useCallback(
    (id: string, el: HTMLButtonElement | null) => {
      resultItemRefs.current[id] = el;
    },
    [resultItemRefs],
  );
  return (
<div className="result-list">
  {hits.length === 0 ? (
    <div
      className={
        !query.trim() && !hasIndexedDocs
          ? "empty onboarding"
          : "empty"
      }
    >
      {!query.trim() && !hasIndexedDocs ? (
        <>
          <div className="onboarding-mark">SD</div>
          <h3>30 秒开始正文搜索</h3>
          <p>
            添加笔记/文档文件夹，建立<strong>正文索引</strong>
            （不是文件名搜索）。Google Docs 可之后在「来源」添加。
          </p>
          <ol className="onboarding-steps">
            <li>选一个本地文件夹</li>
            <li>等待首次建立索引</li>
            <li>搜一句话或关键词，右侧预览</li>
          </ol>
          <div className="empty-actions">
            <button
              type="button"
              className="btn primary"
              onClick={() => void handleAddFolder()}
              disabled={indexBusy}
            >
              {indexBusy ? "索引中…" : "添加文件夹并开始"}
            </button>
            <button
              type="button"
              className="btn ghost sm"
              onClick={() => setTab("sources")}
            >
              打开来源
            </button>
          </div>
          <p className="onboarding-note">
            数据只在本机 · 不是 Everything（我们搜的是正文）
          </p>
        </>
      ) : (
        <>
          <h3>
            {query.trim()
              ? "没有匹配结果"
              : "输入关键词开始搜索"}
          </h3>
          <p>
            {query.trim()
              ? sourceFilter !== "all" ||
                searchScope !== "all" ||
                searchMode === "exact"
                ? "可放宽条件后再试：来源、匹配方式或搜索范围。"
                : "试试更短的词，或用 a OR b。也可到来源同步库。"
              : "点选结果可在右侧预览；双击打开原文。"}
          </p>
          {query.trim() ? (
            <div className="empty-actions">
              <button
                type="button"
                className="btn ghost sm"
                onClick={() => {
                  setQuery("");
                  setHits([]);
                  setSelectedId(null);
                  searchInputRef.current?.focus();
                }}
              >
                清空关键词
              </button>
              {sourceFilter !== "all" && (
                <button
                  type="button"
                  className="btn primary sm"
                  onClick={() => setSourceFilter("all")}
                >
                  全部来源
                </button>
              )}
              {searchMode === "exact" && (
                <button
                  type="button"
                  className="btn ghost sm"
                  onClick={() => setSearchMode("fuzzy")}
                >
                  改用模糊
                </button>
              )}
              {searchScope !== "all" && (
                <button
                  type="button"
                  className="btn ghost sm"
                  onClick={() => {
                    setSearchScope("all");
                    setShowSearchMore(true);
                  }}
                >
                  范围：全部
                </button>
              )}
              {/\bNOT\b|^\s*-|\s-\S/i.test(query) && (
                <button
                  type="button"
                  className="btn ghost sm"
                  onClick={() => {
                    setQuery(
                      query
                        .replace(/\bNOT\b/gi, " ")
                        .replace(/(^|\s)-\S+/g, " ")
                        .replace(/\s+/g, " ")
                        .trim(),
                    );
                    searchInputRef.current?.focus();
                  }}
                >
                  去掉排除词
                </button>
              )}
              <button
                type="button"
                className="btn ghost sm"
                onClick={() => void handleSyncAll()}
                disabled={indexBusy}
              >
                同步库
              </button>
            </div>
          ) : null}
        </>
      )}
    </div>
  ) : (
    hits.map((hit) => (
      <ResultItem
        key={hit.id}
        hit={hit}
        selected={hit.id === selectedId}
        query={query}
        searchMode={searchMode}
        onSelect={handleSelect}
        onOpen={handleOpenStable}
        registerRef={registerRef}
      />
    ))
  )}
  {hits.length > 0 && hasMore && (
    <button
      type="button"
      className="btn ghost sm result-load-more"
      onClick={loadMore}
      disabled={loading}
    >
      {loading ? "加载中…" : "加载更多"}
    </button>
  )}
</div>
  );
}
