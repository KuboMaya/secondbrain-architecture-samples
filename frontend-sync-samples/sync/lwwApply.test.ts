/**
 * Portfolio sample — LWW unit tests (Vitest/Jest style).
 * Mirrors the decision matrix used in Rust conflict_tests.
 */

import { describe, expect, it } from "vitest";
import { lwwAccept } from "./lwwApply";

describe("lwwAccept", () => {
  it("accepts when local is missing", () => {
    expect(lwwAccept(undefined, 1)).toBe(true);
  });

  it("accepts strictly newer remote", () => {
    expect(lwwAccept(1, 2)).toBe(true);
    expect(lwwAccept(10, 1_000_000)).toBe(true);
  });

  it("keeps local on equal or older remote", () => {
    expect(lwwAccept(5, 5)).toBe(false);
    expect(lwwAccept(10, 3)).toBe(false);
  });

  it("force overrides LWW", () => {
    expect(lwwAccept(10, 1, true)).toBe(true);
  });

  it("boundary: n → n+1 accepts", () => {
    expect(lwwAccept(10, 11)).toBe(true);
    expect(lwwAccept(10, 10)).toBe(false);
  });
});
