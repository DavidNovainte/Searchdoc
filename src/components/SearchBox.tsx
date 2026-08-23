import "../styles/SearchBox.css";
import type { Dispatch, RefObject, SetStateAction } from "react";
import type { SearchMode } from "../types";
import { IconSearch } from "../icons";

export interface SearchBoxProps {
  query: string;
  setQuery: Dispatch<SetStateAction<string>>;
  loading: boolean;
  showRecent: boolean;
  setShowRecent: Dispatch<SetStateAction<boolean>>;
  recentSearches: string[];
  clearRecentSearches: () => void;
  searchInputRef: RefObject<HTMLInputElement | null>;
  searchMode: SearchMode;
}

export function SearchBox(props: SearchBoxProps) {
  const {
    query, setQuery, loading, showRecent, setShowRecent,
    recentSearches, clearRecentSearches, searchInputRef, searchMode,
  } = props;
  return (
<div className="search-box-row">
  <div className="search-box-wrap">
    <div className="search-box">
      <span className="search-icon">
        <IconSearch size={18} />
      </span>
      <input
        ref={searchInputRef}
        autoFocus
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onFocus={() => setShowRecent(true)}
        onBlur={() => {
          window.setTimeout(() => setShowRecent(false), 150);
        }}
        placeholder={
          searchMode === "exact"
            ? "精准短语，例如：统一索引"
            : "搜正文…  空格=且  OR=或  NOT/- =排除"
        }
      />
      {loading && <span className="spinner" />}
      {query && (
        <button
          type="button"
          className="clear-btn"
          onClick={() => setQuery("")}
          title="清空"
        >
          ×
        </button>
      )}
    </div>
  {showRecent && !query.trim() && recentSearches.length > 0 && (
      <div className="recent-pop">
        <div className="recent-pop-head">
          <span>最近搜索</span>
          <button
            type="button"
            className="recent-clear"
            onMouseDown={(e) => e.preventDefault()}
            onClick={clearRecentSearches}
          >
            清空
          </button>
        </div>
        {recentSearches.map((item) => (
          <button
            key={item}
            type="button"
            className="recent-item"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => {
              setQuery(item);
              setShowRecent(false);
              searchInputRef.current?.focus();
            }}
          >
            <span className="recent-ico">
              <IconSearch size={14} />
            </span>
            <span className="recent-text">{item}</span>
          </button>
        ))}
      </div>
    )}
  </div>
</div>
  );
}
