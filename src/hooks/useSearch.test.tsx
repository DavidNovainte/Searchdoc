import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useSearch } from "./useSearch";
import * as api from "../lib/api";
import type { DeepSearchStats, SearchHit, SearchResponse } from "../types";

vi.mock("../lib/api", () => ({
  searchDocuments: vi.fn(),
}));

const searchDocumentsMock = vi.mocked(api.searchDocuments);

function hit(id: string): SearchHit {
  return {
    id,
    source_id: "src",
    source_kind: "local",
    title: id,
    uri: `file:///C:/notes/${id}.md`,
    snippet: `snippet for ${id}`,
    rank: -1,
    mtime: null,
    depth: 0,
    via: null,
  };
}

function deepStats(): DeepSearchStats {
  return {
    enabled: false,
    max_depth: 0,
    seeds: 0,
    links_found: 0,
    links_followed: 0,
    fetched: 0,
    linked_hits: 0,
    skipped_existing: 0,
    errors: [],
  };
}

function response(hits: SearchHit[]): SearchResponse {
  return { hits, deep: deepStats() };
}

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

function renderSearch() {
  return renderHook(() =>
    useSearch({ setError: vi.fn(), setStatus: vi.fn() }),
  );
}
type SearchHookRendered = ReturnType<typeof renderSearch>;

function resultHits(rendered: SearchHookRendered): SearchHit[] {
  return rendered.result.current.hits;
}

async function type(rendered: SearchHookRendered, q: string) {
  // Act 1 commits the state change so effects (debounce scheduling) run.
  await act(async () => {
    rendered.result.current.setQuery(q);
    await Promise.resolve();
  });
  // Act 2 advances the clock only after the debounce timer exists.
  await act(async () => {
    await vi.advanceTimersByTimeAsync(180);
    // Yield so real promise continuations (state updates) can settle.
    await Promise.resolve();
    await Promise.resolve();
  });
}

beforeEach(() => {
  // Fake ONLY wall-clock timers; faking microtasks freezes every awaited
  // promise continuation and the debounced fetch never settles.
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval", "Date"] });
  searchDocumentsMock.mockReset();
  localStorage.clear();
});

afterEach(() => {
  vi.useRealTimers();
  cleanup();
});

describe("useSearch", () => {
  it("discards stale responses when the query changes quickly", async () => {
    const first = deferred<SearchResponse>();
    const second = deferred<SearchResponse>();
    searchDocumentsMock
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);

    const rendered = renderSearch();

    await type(rendered, "a");
    await type(rendered, "ab");

    // Old response lands AFTER the newer request was issued.
    await act(async () => {
      first.resolve(response([hit("stale")]));
      await Promise.resolve();
    });
    expect(resultHits(rendered)).toHaveLength(0);

    await act(async () => {
      second.resolve(response([hit("fresh")]));
      await Promise.resolve();
    });
    expect(resultHits(rendered)).toHaveLength(1);
    expect(resultHits(rendered)[0].id).toBe("fresh");
  });

  it("appends cursor pages without resetting the selection", async () => {
    searchDocumentsMock.mockResolvedValueOnce(
      response([hit("one"), hit("two")]),
    );

    const rendered = renderSearch();
    await type(rendered, "roadmap");
    expect(resultHits(rendered)).toHaveLength(2);
    const selectedAfterPageOne = rendered.result.current.selectedId;

    const pageTwo = deferred<SearchResponse>();
    searchDocumentsMock.mockReturnValueOnce(pageTwo.promise);
    await act(async () => {
      rendered.result.current.loadMore();
      await Promise.resolve();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(180);
      await Promise.resolve();
      await Promise.resolve();
    });
    // Selection must survive the in-flight append.
    expect(rendered.result.current.selectedId).toBe(selectedAfterPageOne);

    await act(async () => {
      pageTwo.resolve(response([hit("three")]));
      await Promise.resolve();
    });
    expect(resultHits(rendered)).toHaveLength(3);
    expect(resultHits(rendered).map((h) => h.id)).toEqual([
      "one",
      "two",
      "three",
    ]);
    expect(rendered.result.current.selectedId).toBe(selectedAfterPageOne);
  });

  it("clears results when the query is emptied and reports no more pages", async () => {
    searchDocumentsMock
      .mockResolvedValueOnce(response([hit("only")]))
      .mockResolvedValueOnce(response([]));

    const rendered = renderSearch();
    await type(rendered, "solo");
    expect(resultHits(rendered)).toHaveLength(1);
    expect(rendered.result.current.hasMore).toBe(false); // short page ⇒ end

    await act(async () => {
      rendered.result.current.setQuery("");
      await Promise.resolve();
    });
    expect(resultHits(rendered)).toHaveLength(0);
    expect(searchDocumentsMock).toHaveBeenCalledTimes(1); // empty query skips backend
  });
});
