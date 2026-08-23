import { useEffect, useMemo, useRef, useState } from "react";
import * as api from "../lib/api";
import { debugLog } from "../lib/debugLog";
import { countMatchesInText } from "../lib/highlight";
import type { SearchHit, SearchMode } from "../types";

// Very large bodies stall the highlight/scroll pass; preview only the head.
// Truncating here keeps match counting and rendered text consistent.
const MAX_PREVIEW_CHARS = 120_000;

function truncatePreview(body: string): string {
  if (body.length <= MAX_PREVIEW_CHARS) return body;
  return `${body.slice(0, MAX_PREVIEW_CHARS)}\n\n…（长文档，预览已截断）`;
}

export interface UsePreviewOptions {
  hits: SearchHit[];
  query: string;
  searchMode: SearchMode;
  selectedId: string | null;
  setSelectedId: (value: string | null) => void;
}

export function usePreview(options: UsePreviewOptions) {
  const { hits, query, searchMode, selectedId, setSelectedId } = options;

  const [previewBody, setPreviewBody] = useState<string>("");
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [activeMatch, setActiveMatch] = useState(0);
  const previewScrollRef = useRef<HTMLDivElement | null>(null);
  const resultItemRefs = useRef<Record<string, HTMLButtonElement | null>>({});

  const selected = useMemo(
    () => hits.find((h) => h.id === selectedId) ?? null,
    [hits, selectedId],
  );

  useEffect(() => {
    if (!selectedId) {
      setPreviewBody("");
      setPreviewError(null);
      setPreviewLoading(false);
      setActiveMatch(0);
      return;
    }

    let cancelled = false;
    debugLog("preview.start", { documentId: selectedId });
    setPreviewLoading(true);
    setPreviewError(null);
    setPreviewBody("");
    setActiveMatch(0);

    api.getDocumentPreview(selectedId)
      .then((body) => {
        if (cancelled) return;
        setPreviewBody(body ? truncatePreview(body) : "");
        debugLog("preview.complete", { documentId: selectedId, empty: !body });
        if (!body) setPreviewError("暂无正文预览（可能尚未索引完整内容）");
      })
      .catch((err) => {
        if (cancelled) return;
        debugLog("preview.error", { documentId: selectedId, error: String(err) });
        setPreviewError(String(err));
        setPreviewBody("");
      })
      .finally(() => {
        if (!cancelled) setPreviewLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [selectedId]);

  const matchCount = useMemo(
    () => countMatchesInText(previewBody, query, searchMode),
    [previewBody, query, searchMode],
  );

  useEffect(() => {
    if (matchCount <= 0) {
      setActiveMatch(0);
      return;
    }
    setActiveMatch(0);
  }, [matchCount, query, searchMode, selectedId]);

  useEffect(() => {
    if (matchCount <= 0 || previewLoading) return;
    const container = previewScrollRef.current;
    const el = document.getElementById(`hit-${activeMatch}`);
    if (!container || !el) return;

    const cRect = container.getBoundingClientRect();
    const eRect = el.getBoundingClientRect();
    const offset =
      eRect.top - cRect.top - cRect.height / 2 + eRect.height / 2 + container.scrollTop;
    container.scrollTo({
      top: Math.max(0, offset),
      behavior: activeMatch === 0 ? "auto" : "smooth",
    });
  }, [activeMatch, matchCount, previewBody, previewLoading]);

  useEffect(() => {
    if (!selectedId) return;
    const row = resultItemRefs.current[selectedId];
    row?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [selectedId]);

  function goPrevMatch(e?: React.MouseEvent) {
    e?.preventDefault();
    e?.stopPropagation();
    if (matchCount <= 0) return;
    setActiveMatch((i) => (i - 1 + matchCount) % matchCount);
  }

  function goNextMatch(e?: React.MouseEvent) {
    e?.preventDefault();
    e?.stopPropagation();
    if (matchCount <= 0) return;
    setActiveMatch((i) => (i + 1) % matchCount);
  }

  function selectHitByOffset(delta: number) {
    if (hits.length === 0) return;
    const idx = hits.findIndex((h) => h.id === selectedId);
    const base = idx >= 0 ? idx : 0;
    const next = (base + delta + hits.length) % hits.length;
    setSelectedId(hits[next].id);
  }

  return {
    selected, previewBody, previewLoading, previewError, activeMatch, matchCount,
    previewScrollRef, resultItemRefs,
    goPrevMatch, goNextMatch, selectHitByOffset,
  };
}
