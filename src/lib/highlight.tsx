import "../styles/highlight.css";
import type { ReactNode } from "react";
import type { SearchMode } from "../types";

export function highlightSnippet(snippet: string): ReactNode {
  const parts = snippet.split(/(⟦|⟧)/);
  let active = false;
  return parts.map((part, index) => {
    if (part === "⟦") {
      active = true;
      return null;
    }
    if (part === "⟧") {
      active = false;
      return null;
    }
    if (active) {
      return (
        <mark key={index} className="hit-mark">
          {part}
        </mark>
      );
    }
    return <span key={index}>{part}</span>;
  });
}

export function buildHighlightTokens(query: string, mode: SearchMode): string[] {
  const q = query.trim();
  if (!q) return [];
  if (mode === "exact") {
    return [q];
  }
  const tokens = Array.from(
    new Set(
      q
        .split(/\s+/)
        .map((t) => t.trim())
        .filter((t) => t.length > 0)
        .filter((t) => !(t.length === 1 && /[\u4e00-\u9fff]/.test(t) && q.length > 1))
        .sort((a, b) => b.length - a.length),
    ),
  );
  if (tokens.length === 0 && /[\u4e00-\u9fff]/.test(q)) {
    return [q];
  }
  return tokens;
}

export function countMatchesInText(text: string, query: string, mode: SearchMode): number {
  const tokens = buildHighlightTokens(query, mode);
  if (!text || tokens.length === 0) return 0;
  const escaped = tokens.map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const re = new RegExp(escaped.join("|"), mode === "exact" ? "g" : "gi");
  const matches = text.match(re);
  return matches?.length ?? 0;
}

/**
 * Highlight query inside plain text.
 * - exact: only continuous full query phrase
 * - fuzzy: whitespace tokens; for CJK only whole token, not every single character
 * Match marks get ids hit-0..n for prev/next navigation.
 */
export function highlightPreview(
  text: string,
  query: string,
  mode: SearchMode = "fuzzy",
  activeMatchIndex = 0,
): ReactNode {
  const q = query.trim();
  if (!q || !text) return text;

  const tokens = buildHighlightTokens(query, mode);
  if (tokens.length === 0) return text;

  const escaped = tokens.map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const re = new RegExp(`(${escaped.join("|")})`, mode === "exact" ? "g" : "gi");
  const parts = text.split(re);

  let matchIndex = 0;
  return parts.map((part, index) => {
    if (!part) return null;
    const matched = tokens.some((t) =>
      mode === "exact" ? part === t : part.toLowerCase() === t.toLowerCase(),
    );
    if (matched) {
      const current = matchIndex;
      matchIndex += 1;
      const isActive = current === activeMatchIndex;
      return (
        <mark
          key={index}
          id={`hit-${current}`}
          className={isActive ? "hit-mark hit-mark-active" : "hit-mark"}
          data-hit-index={current}
        >
          {part}
        </mark>
      );
    }
    return <span key={index}>{part}</span>;
  });
}

/** Rebuild list/detail snippet highlights according to current search mode. */
export function highlightSmartSnippet(
  snippet: string,
  query: string,
  mode: SearchMode,
): ReactNode {
  // Backend fuzzy snippets may contain ⟦ ⟧ around single CJK chars.
  // In exact mode, ignore those and only mark the full phrase.
  if (mode === "exact") {
    const plain = snippet.replace(/[⟦⟧]/g, "");
    return highlightPreview(plain, query, "exact");
  }
  // Fuzzy: keep backend markers when present; otherwise fall back to token highlight.
  if (snippet.includes("⟦") && snippet.includes("⟧")) {
    return highlightSnippet(snippet);
  }
  return highlightPreview(snippet, query, "fuzzy");
}