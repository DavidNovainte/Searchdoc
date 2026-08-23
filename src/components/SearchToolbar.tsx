import "../styles/SearchToolbar.css";
import type { Dispatch, RefObject, SetStateAction } from "react";
import type {
  DeepSearchStats,
  IndexStats,
  SearchHit,
  SearchMode,
  SearchScope,
  SearchSort,
  SourceFilter,
} from "../types";
import { SearchBox } from "./SearchBox";
import type { SearchBoxProps } from "./SearchBox";
import { SearchFilters } from "./SearchFilters";
import type { SearchFiltersProps } from "./SearchFilters";
export interface SearchToolbarProps {
  query: string;
  setQuery: Dispatch<SetStateAction<string>>;
  loading: boolean;
  showRecent: boolean;
  setShowRecent: Dispatch<SetStateAction<boolean>>;
  recentSearches: string[];
  clearRecentSearches: () => void;
  searchInputRef: RefObject<HTMLInputElement | null>;
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

export function SearchToolbar(props: SearchToolbarProps) {
  const {
    query, setQuery, loading, showRecent, setShowRecent, recentSearches, clearRecentSearches,
    searchInputRef, searchMode, setSearchMode, sourceFilter, setSourceFilter,
    searchScope, setSearchScope, searchSort, setSearchSort, searchAdvancedActive,
    searchMoreOpen, setShowSearchMore,
    deepSearch, setDeepSearch, deepDepth, setDeepDepth,
    deepLinkLimit, setDeepLinkLimit, deepFetchMissing, setDeepFetchMissing, deepStats,
    hits, stats, hasIndexedDocs,
  } = props;

  const searchBoxProps: SearchBoxProps = {
    query, setQuery, loading, showRecent, setShowRecent,
    recentSearches, clearRecentSearches, searchInputRef, searchMode,
  };
  const searchFiltersProps: SearchFiltersProps = {
    query, loading,
    searchMode, setSearchMode, sourceFilter, setSourceFilter,
    searchScope, setSearchScope, searchSort, setSearchSort, searchAdvancedActive,
    searchMoreOpen, setShowSearchMore,
    deepSearch, setDeepSearch, deepDepth, setDeepDepth,
    deepLinkLimit, setDeepLinkLimit, deepFetchMissing, setDeepFetchMissing, deepStats,
    hits, stats, hasIndexedDocs,
  };

  return (
    <div className="search-toolbar">
      <SearchBox {...searchBoxProps} />
      <SearchFilters {...searchFiltersProps} />
    </div>
  );
}
