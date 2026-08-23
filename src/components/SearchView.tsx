import "../styles/SearchView.css";
import { useEffect, useRef } from "react";
import type { IndexStats } from "../types";
import type { Tab } from "../lib/format";
import { useSearch } from "../hooks/useSearch";
import { usePreview } from "../hooks/usePreview";
import { useOpenActions } from "../hooks/useOpenActions";
import { useKeyboardNav } from "../hooks/useKeyboardNav";
import { SearchToolbar } from "./SearchToolbar";
import type { SearchToolbarProps } from "./SearchToolbar";
import { ResultList } from "./ResultList";
import type { ResultListProps } from "./ResultList";
import { PreviewPanel } from "./PreviewPanel";
import type { PreviewPanelProps } from "./PreviewPanel";

export interface SearchViewProps {
  active: boolean;
  stats: IndexStats | null;
  indexBusy: boolean;
  setTab: (tab: Tab) => void;
  handleAddFolder: () => void;
  handleSyncAll: () => void;
  flash: (message: string) => void;
  setStatus: (message: string) => void;
  setError: (message: string | null) => void;
}

export function SearchView(props: SearchViewProps) {
  const { active, stats, indexBusy, setTab, handleAddFolder, handleSyncAll, flash, setStatus, setError } = props;

  const searchInputRef = useRef<HTMLInputElement | null>(null);

  const { handleOpen, handleReveal, handleCopyPath } = useOpenActions({ flash, setError });

  const search = useSearch({ setError, setStatus });

  const preview = usePreview({
    hits: search.hits,
    query: search.query,
    searchMode: search.searchMode,
    selectedId: search.selectedId,
    setSelectedId: search.setSelectedId,
  });

  useKeyboardNav({
    active,
    query: search.query,
    setQuery: search.setQuery,
    selected: preview.selected,
    handleOpen,
    goPrevMatch: preview.goPrevMatch,
    goNextMatch: preview.goNextMatch,
    selectHitByOffset: preview.selectHitByOffset,
    searchInputRef,
  });

  useEffect(() => {
    if (!active) return;
    const t = window.setTimeout(() => searchInputRef.current?.focus(), 30);
    return () => window.clearTimeout(t);
  }, [active]);

  const hasIndexedDocs = (stats?.document_count ?? 0) > 0;

  const searchToolbarProps: SearchToolbarProps = {
    query: search.query, setQuery: search.setQuery, loading: search.loading,
    showRecent: search.showRecent, setShowRecent: search.setShowRecent,
    recentSearches: search.recentSearches, clearRecentSearches: search.clearRecentSearches,
    searchInputRef, searchMode: search.searchMode, setSearchMode: search.setSearchMode,
    sourceFilter: search.sourceFilter, setSourceFilter: search.setSourceFilter,
    searchScope: search.searchScope, setSearchScope: search.setSearchScope,
    searchSort: search.searchSort, setSearchSort: search.setSearchSort,
    searchMoreOpen: search.searchMoreOpen, setShowSearchMore: search.setShowSearchMore,
    searchAdvancedActive: search.searchAdvancedActive,
    deepSearch: search.deepSearch, setDeepSearch: search.setDeepSearch,
    deepDepth: search.deepDepth, setDeepDepth: search.setDeepDepth,
    deepLinkLimit: search.deepLinkLimit, setDeepLinkLimit: search.setDeepLinkLimit,
    deepFetchMissing: search.deepFetchMissing, setDeepFetchMissing: search.setDeepFetchMissing,
    deepStats: search.deepStats,
    hits: search.hits, stats, hasIndexedDocs,
  };
  const resultListProps: ResultListProps = {
    hits: search.hits, setHits: search.setHits,
    selectedId: search.selectedId, setSelectedId: search.setSelectedId,
    query: search.query, setQuery: search.setQuery,
    resultItemRefs: preview.resultItemRefs, setTab, handleOpen, handleAddFolder, handleSyncAll,
    indexBusy, hasIndexedDocs,
    searchMode: search.searchMode, setSearchMode: search.setSearchMode,
    sourceFilter: search.sourceFilter, setSourceFilter: search.setSourceFilter,
    searchScope: search.searchScope, setSearchScope: search.setSearchScope,
    setShowSearchMore: search.setShowSearchMore, searchInputRef,
    hasMore: search.hasMore, loading: search.loading, loadMore: search.loadMore,
  };
  const previewPanelProps: PreviewPanelProps = {
    selected: preview.selected,
    previewLoading: preview.previewLoading, previewBody: preview.previewBody,
    previewError: preview.previewError, activeMatch: preview.activeMatch, matchCount: preview.matchCount,
    query: search.query, searchMode: search.searchMode,
    goPrevMatch: preview.goPrevMatch, goNextMatch: preview.goNextMatch,
    handleOpen, handleReveal, handleCopyPath, previewScrollRef: preview.previewScrollRef,
  };

  return (
    <div className="search-layout">
      <section className="panel search-panel">
        <SearchToolbar {...searchToolbarProps} />
        <ResultList {...resultListProps} />
      </section>
      <PreviewPanel {...previewPanelProps} />
    </div>
  );
}
