import type { SourceKind } from "../types";

export type { SourceKind };

export type Tab = "search" | "sources" | "settings";
export type SettingsSection = "google" | "sync" | "system" | "data" | "about";

export function sourceLabel(kind: SourceKind): string {
  if (kind === "google_docs") return "Google Docs";
  if (kind === "notion") return "Notion";
  return "本地";
}

export function isWebUri(uri: string): boolean {
  try {
    const protocol = new URL(uri).protocol;
    return protocol === "http:" || protocol === "https:";
  } catch {
    return false;
  }
}

export function formatTime(value: string | null): string {
  if (!value) return "尚未同步";
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return value;
  return d.toLocaleString();
}

export function shortPath(uri: string): string {
  if (!uri) return "";
  if (isWebUri(uri)) {
    try {
      const u = new URL(uri);
      return u.host + u.pathname.replace(/\/edit.*$/, "");
    } catch {
      return uri;
    }
  }
  const norm = uri.replace(/\\/g, "/");
  const parts = norm.split("/").filter(Boolean);
  if (parts.length <= 2) return uri;
  // parent folder + file name
  return parts.slice(-2).join("/");
}

export function parentDir(uri: string): string | null {
  if (!uri || isWebUri(uri)) return null;
  const norm = uri.replace(/\\/g, "/");
  const idx = norm.lastIndexOf("/");
  if (idx <= 0) return null;
  if (/^[a-zA-Z]:\/$/.test(norm.slice(0, idx + 1))) {
    return uri.slice(0, 3);
  }
  // Keep Windows drive path shape when possible
  const dir = uri.includes("\\")
    ? uri.slice(0, uri.replace(/\//g, "\\").lastIndexOf("\\"))
    : uri.slice(0, idx);
  return dir || null;
}

export function isWindowsDriveRoot(path: string): boolean {
  return /^[a-zA-Z]:[\\/]?$/.test(path.trim());
}

export function relativeTime(value: string | null): string {
  if (!value) return "";
  const t = new Date(value).getTime();
  if (Number.isNaN(t)) return "";
  const diff = Date.now() - t;
  if (diff < 0) return "刚刚";
  const sec = Math.floor(diff / 1000);
  if (sec < 60) return "刚刚";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min} 分钟前`;
  const hr = Math.floor(min / 60);
  if (hr < 48) return `${hr} 小时前`;
  const day = Math.floor(hr / 24);
  if (day < 30) return `${day} 天前`;
  return new Date(value).toLocaleDateString();
}

export function tabTitle(tab: Tab): string {
  if (tab === "search") return "搜索";
  if (tab === "sources") return "来源";
  return "设置";
}

export function tabSubtitle(tab: Tab, settingsSection?: SettingsSection): string {
  if (tab === "search") return "正文检索 · 空格且 · OR · NOT/-排除";
  if (tab === "sources") return "要搜哪些库：文件夹 · 磁盘 · Docs";
  switch (settingsSection) {
    case "sync":
      return "Google 同步哪些文档";
    case "system":
      return "启动与常驻";
    case "data":
      return "索引与凭据都在本机";
    case "about":
      return "跨源正文搜索 · 不是文件名搜索";
    default:
      return "Google 可选，本地即可用";
  }
}
