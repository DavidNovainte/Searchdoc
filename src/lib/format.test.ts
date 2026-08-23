import { describe, expect, it } from "vitest";
import { isWebUri, parentDir } from "./format";

describe("path and URL boundaries", () => {
  it("accepts only HTTP(S) browser links", () => {
    expect(isWebUri("https://docs.google.com/document/d/abc/edit")).toBe(true);
    expect(isWebUri("http://localhost:8080/doc")).toBe(true);
    expect(isWebUri("javascript:alert(1)")).toBe(false);
    expect(isWebUri("http-not-a-url")).toBe(false);
  });

  it("keeps the separator for files directly under a Windows drive", () => {
    expect(parentDir("C:\\note.txt")).toBe("C:\\");
    expect(parentDir("C:\\notes\\note.txt")).toBe("C:\\notes");
    expect(parentDir("https://example.com/note.txt")).toBeNull();
  });
});
