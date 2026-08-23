export function debugLog(event: string, details: unknown = {}) {
  if (!import.meta.env.DEV) return;
  console.debug(`[SearchDoc] ${new Date().toISOString()} ${event}`, details);
}
