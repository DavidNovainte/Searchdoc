import { describe, expect, it } from "vitest";
import {
  canLoadMore,
  nextOffset,
  SEARCH_MAX_LIMIT,
  SEARCH_PAGE_SIZE,
} from "./searchPagination";

describe("cursor pagination", () => {
  it("advances the offset by page size up to the ceiling", () => {
    expect(SEARCH_PAGE_SIZE).toBe(50);
    expect(nextOffset(0)).toBe(SEARCH_PAGE_SIZE);
    expect(nextOffset(450)).toBe(SEARCH_MAX_LIMIT);
    expect(nextOffset(SEARCH_MAX_LIMIT)).toBe(SEARCH_MAX_LIMIT);
  });

  it("offers more only when the last page was full and the ceiling is not hit", () => {
    expect(canLoadMore("abc", 50, SEARCH_PAGE_SIZE)).toBe(true);
    expect(canLoadMore("abc", 60, SEARCH_PAGE_SIZE)).toBe(true);
    expect(canLoadMore("abc", 30, 30)).toBe(false);
    expect(canLoadMore("", 0, SEARCH_PAGE_SIZE)).toBe(false);
    expect(canLoadMore("abc", 500, SEARCH_PAGE_SIZE)).toBe(false);
  });
});
