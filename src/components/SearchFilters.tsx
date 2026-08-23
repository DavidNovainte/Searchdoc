import "../styles/SearchFilters.css";
import type { Dispatch, SetStateAction } from "react";
import type {
  DeepSearchStats,
  IndexStats,
  SearchHit,
  SearchMode,
  SearchScope,
  SearchSort,
  SourceFilter,
} from "../types";

export interface SearchFiltersProps {
  query: string;
  loading: boolean;
  searchMode: SearchMode;
  setSearchMode: Dispatch<SetStateAction<SearchMode>>;
  sourceFilter: SourceFilter;
  setSourceFilter: Dispatch<SetStateAction<SourceFilter>>;
  searchScope: SearchScope;
  setSearchScope: Dispatch<SetStateAction<SearchScope>>;
  searchSort: SearchSort;
  setSearchSort: Dispatch<SetStateAction<SearchSort>>;
  searchMoreOpen: boolean;
  setShowSearchMore: Dispatch<SetStateAction<boolean>>;
  searchAdvancedActive: boolean;
  deepSearch: boolean;
  setDeepSearch: Dispatch<SetStateAction<boolean>>;
  deepDepth: 1 | 2;
  setDeepDepth: Dispatch<SetStateAction<1 | 2>>;
  deepLinkLimit: 8 | 12 | 24;
  setDeepLinkLimit: Dispatch<SetStateAction<8 | 12 | 24>>;
  deepFetchMissing: boolean;
  setDeepFetchMissing: Dispatch<SetStateAction<boolean>>;
  deepStats: DeepSearchStats | null;
  hits: SearchHit[];
  stats: IndexStats | null;
  hasIndexedDocs: boolean;
}

export function SearchFilters(props: SearchFiltersProps) {
  const {
    query, loading,
    searchMode, setSearchMode, sourceFilter, setSourceFilter,
    searchScope, setSearchScope, searchSort, setSearchSort, searchAdvancedActive,
    searchMoreOpen, setShowSearchMore,
    deepSearch, setDeepSearch, deepDepth, setDeepDepth,
    deepLinkLimit, setDeepLinkLimit, deepFetchMissing, setDeepFetchMissing, deepStats,
    hits, stats, hasIndexedDocs,
  } = props;
  return (
    <>
<div className="filter-bar">
  <div className="filter-primary">
  <div className="filter-group" role="tablist" aria-label="匹配模式">
    <span className="filter-label">匹配</span>
    {(
      [
        ["fuzzy", "模糊"],
        ["exact", "精准"],
      ] as const
    ).map(([value, label]) => (
      <button
        key={value}
        type="button"
        className={
          searchMode === value ? "filter-chip active" : "filter-chip"
        }
        onClick={() => setSearchMode(value)}
        title={
          value === "exact"
            ? "短语连续匹配"
            : "分词宽松匹配；可用 a OR b"
        }
      >
        {label}
      </button>
    ))}
  </div>
  <div className="filter-group" role="tablist" aria-label="来源">
    <span className="filter-label">来源</span>
    {(
      [
        ["all", "全部"],
        ["local", "本地"],
        ["google_docs", "Google"],
        ["notion", "Notion"],
      ] as const
    ).map(([value, label]) => (
      <button
        key={value}
        type="button"
        className={
          sourceFilter === value ? "filter-chip active" : "filter-chip"
        }
        onClick={() => setSourceFilter(value)}
      >
        {label}
      </button>
    ))}
  </div>
  </div>
  <div className="filter-group trailing">
    <button
      type="button"
      className={
        searchMoreOpen || searchAdvancedActive
          ? "filter-chip active"
          : "filter-chip"
      }
      onClick={() => setShowSearchMore((v) => !v)}
      title="范围 · 排序 · 深度"
      aria-expanded={searchMoreOpen}
    >
      高级筛选
      {searchAdvancedActive ? " ·" : ""}
      {searchScope !== "all"
        ? searchScope === "title"
          ? "标题"
          : "正文"
        : ""}
      {searchSort === "mtime" ? "最新" : ""}
      {deepSearch ? "深" : ""}
    </button>
    <span className="filter-meta">
      {loading
        ? "搜索中…"
        : query.trim()
          ? `${hits.length} 条`
          : hasIndexedDocs
            ? `${stats?.document_count ?? 0} 篇`
            : "未索引"}
    </span>
  </div>
</div>
{searchMoreOpen && (
  <div className="filter-more">
    <div className="filter-bar nested">
      <div className="filter-group" role="tablist" aria-label="搜索范围">
        <span className="filter-label">范围</span>
        {(
          [
            ["all", "全部"],
            ["title", "标题"],
            ["body", "正文"],
          ] as const
        ).map(([value, label]) => (
          <button
            key={value}
            type="button"
            className={
              searchScope === value
                ? "filter-chip active"
                : "filter-chip"
            }
            onClick={() => setSearchScope(value)}
          >
            {label}
          </button>
        ))}
      </div>
      <div className="filter-group" role="tablist" aria-label="排序">
        <span className="filter-label">排序</span>
        {(
          [
            ["relevance", "相关"],
            ["mtime", "最新"],
          ] as const
        ).map(([value, label]) => (
          <button
            key={value}
            type="button"
            className={
              searchSort === value
                ? "filter-chip active"
                : "filter-chip"
            }
            onClick={() => setSearchSort(value)}
          >
            {label}
          </button>
        ))}
      </div>
      <div className="filter-group">
        <span className="filter-label">深度</span>
        <button
          type="button"
          className={deepSearch ? "filter-chip active" : "filter-chip"}
          onClick={() => setDeepSearch((v) => !v)}
          title="跟随 Google Docs 外链"
        >
          {deepSearch ? "已开启" : "关闭"}
        </button>
      </div>
      {searchAdvancedActive && (
        <button
          type="button"
          className="filter-reset"
          onClick={() => {
            setSearchScope("all");
            setSearchSort("relevance");
            setDeepSearch(false);
            setShowSearchMore(false);
          }}
        >
          重置
        </button>
      )}
    </div>
    {deepSearch && (
      <div className="deep-panel">
        <div className="deep-controls">
          <label className="deep-field">
            <span>层数</span>
            <select
              value={deepDepth}
              onChange={(e) =>
                setDeepDepth(Number(e.target.value) === 2 ? 2 : 1)
              }
            >
              <option value={1}>1</option>
              <option value={2}>2</option>
            </select>
          </label>
          <label className="deep-field">
            <span>限额</span>
            <select
              value={deepLinkLimit}
              onChange={(e) => {
                const n = Number(e.target.value);
                setDeepLinkLimit(n === 8 ? 8 : n === 24 ? 24 : 12);
              }}
            >
              <option value={8}>8</option>
              <option value={12}>12</option>
              <option value={24}>24</option>
            </select>
          </label>
          <label className="deep-check">
            <input
              type="checkbox"
              checked={deepFetchMissing}
              onChange={(e) => setDeepFetchMissing(e.target.checked)}
            />
            <span>拉取未索引</span>
          </label>
          {deepStats?.enabled && (
            <span className="deep-stats inline">
              链 {deepStats.links_followed}/{deepStats.links_found} · 拉{" "}
              {deepStats.fetched} · 命中 {deepStats.linked_hits}
            </span>
          )}
        </div>
      </div>
    )}
  </div>
)}
    </>
  );
}
