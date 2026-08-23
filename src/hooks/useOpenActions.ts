import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { isWebUri, parentDir } from "../lib/format";
import type { SearchHit } from "../types";

export interface UseOpenActionsOptions {
  flash: (message: string) => void;
  setError: (message: string | null) => void;
}

export function useOpenActions(options: UseOpenActionsOptions) {
  const { flash, setError } = options;

  async function handleOpen(hit: SearchHit) {
    try {
      if (isWebUri(hit.uri)) {
        await openUrl(hit.uri);
      } else if (hit.source_kind === "google_docs") {
        throw new Error("Google 文档链接无效，已阻止打开");
      } else {
        await openPath(hit.uri);
      }
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleReveal(hit: SearchHit) {
    try {
      if (isWebUri(hit.uri)) {
        await openUrl(hit.uri);
        return;
      }
      if (hit.source_kind === "google_docs") {
        throw new Error("Google 文档链接无效，已阻止打开");
      }
      await revealItemInDir(hit.uri);
    } catch (err) {
      try {
        const dir = parentDir(hit.uri);
        if (dir) await openPath(dir);
        else setError(String(err));
      } catch (err2) {
        setError(String(err2));
      }
    }
  }

  async function handleCopyPath(uri: string) {
    try {
      await navigator.clipboard.writeText(uri);
      flash("路径已复制");
    } catch (err) {
      setError(String(err));
    }
  }

  return { handleOpen, handleReveal, handleCopyPath };
}
