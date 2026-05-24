import { describe, expect, it } from "vitest";
import { parseDiff } from "./parse-diff";

describe("parseDiff", () => {
  it("returns empty for empty input", () => {
    expect(parseDiff("")).toEqual([]);
  });

  it("parses a single modified file with additions and deletions", () => {
    const raw = [
      "diff --git a/src/foo.ts b/src/foo.ts",
      "index 1111..2222 100644",
      "--- a/src/foo.ts",
      "+++ b/src/foo.ts",
      "@@ -1,3 +1,4 @@",
      " unchanged",
      "-removed",
      "+added one",
      "+added two",
      " trailing",
    ].join("\n");
    const out = parseDiff(raw);
    expect(out).toHaveLength(1);
    expect(out[0].oldPath).toBe("src/foo.ts");
    expect(out[0].newPath).toBe("src/foo.ts");
    expect(out[0].isNewFile).toBe(false);
    expect(out[0].isDeletedFile).toBe(false);
    expect(out[0].additions).toBe(2);
    expect(out[0].deletions).toBe(1);
    expect(out[0].hunks).toHaveLength(1);
    expect(out[0].hunks[0].header).toBe("@@ -1,3 +1,4 @@");
  });

  it("detects new files via --- /dev/null", () => {
    const raw = [
      "diff --git a/new.ts b/new.ts",
      "new file mode 100644",
      "--- /dev/null",
      "+++ b/new.ts",
      "@@ -0,0 +1,2 @@",
      "+line1",
      "+line2",
    ].join("\n");
    const out = parseDiff(raw);
    expect(out).toHaveLength(1);
    expect(out[0].isNewFile).toBe(true);
    expect(out[0].additions).toBe(2);
    expect(out[0].deletions).toBe(0);
  });

  it("detects deleted files via +++ /dev/null", () => {
    const raw = [
      "diff --git a/gone.ts b/gone.ts",
      "deleted file mode 100644",
      "--- a/gone.ts",
      "+++ /dev/null",
      "@@ -1,2 +0,0 @@",
      "-line1",
      "-line2",
    ].join("\n");
    const out = parseDiff(raw);
    expect(out[0].isDeletedFile).toBe(true);
    expect(out[0].deletions).toBe(2);
  });

  it("flags binary files", () => {
    const raw = [
      "diff --git a/img.png b/img.png",
      "index 11..22 100644",
      "Binary files a/img.png and b/img.png differ",
    ].join("\n");
    const out = parseDiff(raw);
    expect(out[0].isBinary).toBe(true);
  });

  it("parses multiple files in one diff", () => {
    const raw = [
      "diff --git a/a.ts b/a.ts",
      "--- a/a.ts",
      "+++ b/a.ts",
      "@@ -1 +1 @@",
      "-old",
      "+new",
      "diff --git a/b.ts b/b.ts",
      "--- a/b.ts",
      "+++ b/b.ts",
      "@@ -1,2 +1,3 @@",
      " keep",
      "+added",
      " more",
    ].join("\n");
    const out = parseDiff(raw);
    expect(out).toHaveLength(2);
    expect(out[0].newPath).toBe("a.ts");
    expect(out[1].newPath).toBe("b.ts");
    expect(out[1].additions).toBe(1);
  });

  it("does not count the +++ header line as an addition", () => {
    const raw = [
      "diff --git a/x.ts b/x.ts",
      "--- a/x.ts",
      "+++ b/x.ts",
      "@@ -1 +1 @@",
      "-a",
      "+b",
    ].join("\n");
    const out = parseDiff(raw);
    expect(out[0].additions).toBe(1);
    expect(out[0].deletions).toBe(1);
  });
});
