import { useEffect } from "react";
import type { RefObject } from "react";
import type { SearchHit } from "../types";

export interface UseKeyboardNavOptions {
  active: boolean;
  query: string;
  setQuery: (value: string) => void;
  selected: SearchHit | null;
  handleOpen: (hit: SearchHit) => void;
  goPrevMatch: () => void;
  goNextMatch: () => void;
  selectHitByOffset: (delta: number) => void;
  searchInputRef: RefObject<HTMLInputElement | null>;
}

export function useKeyboardNav(options: UseKeyboardNavOptions) {
  const {
    active, query, setQuery, selected, handleOpen,
    goPrevMatch, goNextMatch, selectHitByOffset, searchInputRef,
  } = options;

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (!active) return;

      const target = e.target as HTMLElement | null;
      const tag = target?.tagName?.toLowerCase();
      const typing =
        tag === "input" ||
        tag === "textarea" ||
        !!target?.isContentEditable;

      if (e.key === "F3") {
        e.preventDefault();
        if (e.shiftKey) goPrevMatch();
        else goNextMatch();
        return;
      }

      if (e.ctrlKey && !e.altKey && e.key.toLowerCase() === "g") {
        e.preventDefault();
        if (e.shiftKey) goPrevMatch();
        else goNextMatch();
        return;
      }

      if (e.key === "Escape") {
        if (typing && tag === "input") {
          if (query) {
            e.preventDefault();
            setQuery("");
          }
        }
        return;
      }

      if (typing) {
        if (e.key === "Enter" && e.ctrlKey && selected) {
          e.preventDefault();
          void handleOpen(selected);
        }
        return;
      }

      if (e.key === "ArrowDown") {
        e.preventDefault();
        selectHitByOffset(1);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        selectHitByOffset(-1);
        return;
      }
      if (e.key === "Enter" && selected) {
        e.preventDefault();
        void handleOpen(selected);
        return;
      }
      if (e.key === "/" && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });
}