export interface Hunk {
  header: string;
  lines: string[];
}

export interface FileDiff {
  oldPath: string | null;
  newPath: string | null;
  isNewFile: boolean;
  isDeletedFile: boolean;
  isBinary: boolean;
  additions: number;
  deletions: number;
  hunks: Hunk[];
}

export function parseDiff(raw: string): FileDiff[] {
  const files: FileDiff[] = [];
  const lines = raw.split("\n");

  let current: FileDiff | null = null;
  let currentHunk: Hunk | null = null;

  const push = () => {
    if (current) {
      if (currentHunk) current.hunks.push(currentHunk);
      currentHunk = null;
      files.push(current);
    }
  };

  for (const line of lines) {
    if (line.startsWith("diff --git ")) {
      push();
      const match = line.match(/diff --git a\/(.+?) b\/(.+)$/);
      current = {
        oldPath: match?.[1] ?? null,
        newPath: match?.[2] ?? null,
        isNewFile: false,
        isDeletedFile: false,
        isBinary: false,
        additions: 0,
        deletions: 0,
        hunks: [],
      };
      currentHunk = null;
      continue;
    }
    if (!current) continue;

    if (line.startsWith("new file mode")) {
      current.isNewFile = true;
    } else if (line.startsWith("deleted file mode")) {
      current.isDeletedFile = true;
    } else if (line.startsWith("--- ")) {
      const p = line.slice(4);
      if (p === "/dev/null") current.isNewFile = true;
      else current.oldPath = p.replace(/^a\//, "");
    } else if (line.startsWith("+++ ")) {
      const p = line.slice(4);
      if (p === "/dev/null") current.isDeletedFile = true;
      else current.newPath = p.replace(/^b\//, "");
    } else if (line.startsWith("Binary file")) {
      current.isBinary = true;
    } else if (line.startsWith("@@")) {
      if (currentHunk) current.hunks.push(currentHunk);
      currentHunk = { header: line, lines: [] };
    } else if (currentHunk) {
      currentHunk.lines.push(line);
      if (line.startsWith("+") && !line.startsWith("+++")) {
        current.additions++;
      } else if (line.startsWith("-") && !line.startsWith("---")) {
        current.deletions++;
      }
    }
  }
  push();
  return files;
}
