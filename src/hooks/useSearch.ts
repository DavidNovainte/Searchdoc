import { useEffect, useRef, useState } from "react";
import * as api from "../lib/api";
import { debugLog } from "../lib/debugLog";
import type {
  DeepSearchStats,
  SearchHit,
  SearchMode,
  SearchScope,
  SearchSort,
  SourceFilter,
} from "../types";
import {
  canLoadMore,
  nextOffset,
  SEARCH_PAGE_SIZE,
} from "../lib/searchPagination";

const RECENT_SEARCHES_KEY = "searchdoc.recentSearches";
const MAX_RECENT_SEARCHES = 8;

function loadRecentSearches(): string[] {
  try {
    const raw = localStorage.getItem(RECENT_SEARCHES_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((x) => typeof x === "string" && x.trim())
      .map((x: string) => x.trim())
      .slice(0, MAX_RECENT_SEARCHES);
  } catch {
    return [];
  }
}

function saveRecentSearches(items: string[]) {
  try {
    localStorage.setItem(
      RECENT_SEARCHES_KEY,
      JSON.stringify(items.slice(0, MAX_RECENT_SEARCHES)),
    );
  } catch {
    // Recent searches are optional; private storage failures must not break search.
  }
}

export interface UseSearchOptions {
  setError: (message: string | null) => void;
  setStatus: (message: string) => void;
}

export function useSearch(options: UseSearchOptions) {
  const { setError, setStatus } = options;

  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [offset, setOffset] = useState(0);
  const [lastPageCount, setLastPageCount] = useState(0);
  const [loading, setLoading] = useState(false);
  const [recentSearches, setRecentSearches] = useState<string[]>(() =>
    loadRecentSearches(),
  );
  const [showRecent, setShowRecent] = useState(false);
  const [sourceFilter, setSourceFilter] = useState<SourceFilter>("all");
  const [searchMode, setSearchMode] = useState<SearchMode>("fuzzy");
  const [searchScope, setSearchScope] = useState<SearchScope>("all");
  const [searchSort, setSearchSort] = useState<SearchSort>("relevance");
  const [showSearchMore, setShowSearchMore] = useState(false);
  const [deepSearch, setDeepSearch] = useState(false);
  const [deepDepth, setDeepDepth] = useState<1 | 2>(1);
  const [deepLinkLimit, setDeepLinkLimit] = useState<8 | 12 | 24>(12);
  const [deepFetchMissing, setDeepFetchMissing] = useState(false);
  const [deepStats, setDeepStats] = useState<DeepSearchStats | null>(null);
  const requestSeq = useRef(0);

  function pushRecentSearch(q: string) {
    const term = q.trim();
    if (!term) return;
    setRecentSearches((prev) => {
      const next = [term, ...prev.filter((x) => x !== term)].slice(
        0,
        MAX_RECENT_SEARCHES,
      );
      saveRecentSearches(next);
      return next;
    });
  }

  function clearRecentSearches() {
    setRecentSearches([]);
    saveRecentSearches([]);
    setShowRecent(false);
  }

  const searchAdvancedActive =
    searchScope !== "all" || searchSort !== "relevance" || deepSearch;
  const searchMoreOpen = showSearchMore || searchAdvancedActive;

  // Any filter/query change restarts from the first page.
  useEffect(() => {
    setOffset(0);
  }, [
    query,
    sourceFilter,
    searchMode,
    searchScope,
    searchSort,
    deepSearch,
    deepDepth,
    deepLinkLimit,
    deepFetchMissing,
  ]);

  useEffect(() => {
    const q = query.trim();
    const requestId = ++requestSeq.current;
    if (!q) {
      setHits([]);
      setSelectedId(null);
      setDeepStats(null);
      setLoading(false);
      return;
    }

    const timer = window.setTimeout(async () => {
      setLoading(true);
      setError(null);
      debugLog("search.start", {
        requestId,
        query: q,
        offset,
        sourceFilter,
        searchMode,
        searchScope,
        searchSort,
        deepSearch,
      });
      try {
        const result = await api.searchDocuments({
          query: q,
          limit: SEARCH_PAGE_SIZE,
          offset,
          sourceFilter,
          mode: searchMode,
          scope: searchScope,
          sort: searchSort,
          deep: deepSearch,
          deepDepth,
          deepLinkLimit,
          deepFetchMissing,
        });
        if (requestId !== requestSeq.current) {
          debugLog("search.stale_response", { requestId });
          return;
        }
        const list = result.hits ?? [];
        if (offset > 0) {
          // Cursor page: append so selection and scroll stay put.
          setHits((prev) => [...prev, ...list]);
        } else {
          setHits(list);
          setSelectedId(list[0]?.id ?? null);
        }
        setLastPageCount(list.length);
        setDeepStats(result.deep ?? null);
        pushRecentSearch(q);
        const filterLabel =
          sourceFilter === "all"
            ? "全部"
            : sourceFilter === "local"
              ? "本地"
              : "Google";
        const modeLabel = searchMode === "exact" ? "精准" : "模糊";
        const scopeLabel =
          searchScope === "title" ? "标题" : searchScope === "body" ? "正文" : "";
        const sortLabel = searchSort === "mtime" ? "最新" : "";
        const deepCount = list.filter((h) => (h.depth ?? 0) > 0).length;
        const deepLabel = deepSearch
          ? deepCount > 0
            ? `深${deepDepth}·外链${deepCount}`
            : `深${deepDepth}`
          : "";
        setStatus(
          [`${offset + list.length} 条`, filterLabel, modeLabel, scopeLabel, sortLabel, deepLabel]
            .filter(Boolean)
            .join(" · "),
        );
        debugLog("search.complete", { requestId, hits: list.length });
      } catch (err) {
        if (requestId !== requestSeq.current) return;
        debugLog("search.error", { requestId, error: String(err) });
        setError(String(err));
        setHits([]);
        setDeepStats(null);
      } finally {
        if (requestId === requestSeq.current) setLoading(false);
      }
    }, 180);

    return () => window.clearTimeout(timer);
  }, [
    query,
    sourceFilter,
    searchMode,
    searchScope,
    searchSort,
    deepSearch,
    deepDepth,
    deepLinkLimit,
    deepFetchMissing,
    offset,
  ]);

  function loadMore() {
    if (loading) return;
    setOffset((o) => nextOffset(o));
  }

  return {
    query, setQuery, hits, setHits, loading,
    hasMore: canLoadMore(query, hits.length, lastPageCount),
    loadMore,
    selectedId, setSelectedId,
    recentSearches, showRecent, setShowRecent, clearRecentSearches,
    sourceFilter, setSourceFilter, searchMode, setSearchMode,
    searchScope, setSearchScope, searchSort, setSearchSort,
    showSearchMore, setShowSearchMore,
    deepSearch, setDeepSearch, deepDepth, setDeepDepth,
    deepLinkLimit, setDeepLinkLimit, deepFetchMissing, setDeepFetchMissing, deepStats,
    searchAdvancedActive, searchMoreOpen,
  };
}
