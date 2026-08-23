export const SEARCH_PAGE_SIZE = 50;
export const SEARCH_MAX_LIMIT = 500;

/** Offset cursor for the next page; the backend now treats limit as page size. */
export function nextOffset(offset: number): number {
  return Math.min(offset + SEARCH_PAGE_SIZE, SEARCH_MAX_LIMIT);
}

/** Another page is worth fetching when the last one came back full and the
 * accumulated result set is still under the hard ceiling. */
export function canLoadMore(
  query: string,
  hitCount: number,
  lastPageCount: number,
): boolean {
  return (
    Boolean(query.trim()) &&
    lastPageCount >= SEARCH_PAGE_SIZE &&
    hitCount < SEARCH_MAX_LIMIT
  );
}
