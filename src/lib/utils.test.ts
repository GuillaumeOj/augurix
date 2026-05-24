import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { basename, cn, relativeTime } from "./utils";

describe("cn", () => {
  it("joins truthy classnames", () => {
    expect(cn("a", "b")).toBe("a b");
  });
  it("filters falsy values", () => {
    expect(cn("a", false, undefined, null, "b")).toBe("a b");
  });
  it("merges conflicting tailwind classes via tailwind-merge", () => {
    expect(cn("p-2", "p-4")).toBe("p-4");
  });
});

describe("basename", () => {
  it("returns last segment of a posix path", () => {
    expect(basename("/Users/g/dev/augurix")).toBe("augurix");
  });
  it("ignores trailing slash", () => {
    expect(basename("/Users/g/dev/augurix/")).toBe("augurix");
  });
  it("handles windows-style paths", () => {
    expect(basename("C:\\Users\\g\\dev\\augurix")).toBe("augurix");
  });
  it("returns input when no separator", () => {
    expect(basename("augurix")).toBe("augurix");
  });
});

describe("relativeTime", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-05-24T12:00:00Z"));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("returns em-dash for null/undefined", () => {
    expect(relativeTime(null)).toBe("—");
    expect(relativeTime(undefined)).toBe("—");
  });
  it("returns em-dash for unparsable strings", () => {
    expect(relativeTime("not-a-date")).toBe("—");
  });
  it("'just now' under 5 seconds", () => {
    expect(relativeTime("2026-05-24T11:59:57Z")).toBe("just now");
  });
  it("seconds 5..59", () => {
    expect(relativeTime("2026-05-24T11:59:50Z")).toBe("10s ago");
  });
  it("minutes", () => {
    expect(relativeTime("2026-05-24T11:55:00Z")).toBe("5m ago");
  });
  it("hours", () => {
    expect(relativeTime("2026-05-24T09:00:00Z")).toBe("3h ago");
  });
  it("days", () => {
    expect(relativeTime("2026-05-22T12:00:00Z")).toBe("2d ago");
  });
});
