import {
  ArrowDownIcon,
  ArrowUpIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  GitBranchIcon,
} from "lucide-react";
import { useState } from "react";
import { type DiffScope, useGitDiff } from "@/lib/queries";
import type { GitStatus } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { DiffView } from "./diff-view";

export function GitPanel({ git, projectPath }: { git: GitStatus; projectPath: string }) {
  const [openSections, setOpenSections] = useState<Record<DiffScope, boolean>>({
    unstaged: true,
    staged: false,
    untracked: false,
  });
  const toggle = (s: DiffScope) => setOpenSections((cur) => ({ ...cur, [s]: !cur[s] }));

  if (!git.is_repo) {
    return (
      <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-4 text-sm text-[var(--color-fg-muted)]">
        Not a git repository.
      </div>
    );
  }

  const counts: { label: string; n: number; tone: string }[] = [
    {
      label: "modified",
      n: git.modified,
      tone: "text-[var(--color-warning)]",
    },
    { label: "staged", n: git.staged, tone: "text-[var(--color-success)]" },
    {
      label: "untracked",
      n: git.untracked,
      tone: "text-[var(--color-info)]",
    },
    {
      label: "conflicted",
      n: git.conflicted,
      tone: "text-[var(--color-danger)]",
    },
  ];

  return (
    <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)]">
      <div className="flex items-center gap-3 border-b border-[var(--color-border)] px-4 py-3">
        <GitBranchIcon size={14} className="text-[var(--color-fg-muted)]" />
        <span className="font-mono text-[13px] text-[var(--color-fg)]">
          {git.branch || "detached"}
        </span>
        {git.upstream && (
          <span className="text-[11px] font-mono text-[var(--color-fg-subtle)]">
            → {git.upstream}
          </span>
        )}
        <div className="ml-auto flex items-center gap-3 text-[12px] text-[var(--color-fg-muted)]">
          {git.ahead > 0 && (
            <span className="inline-flex items-center gap-0.5 text-[var(--color-info)]">
              <ArrowUpIcon size={11} />
              {git.ahead}
            </span>
          )}
          {git.behind > 0 && (
            <span className="inline-flex items-center gap-0.5 text-[var(--color-warning)]">
              <ArrowDownIcon size={11} />
              {git.behind}
            </span>
          )}
        </div>
      </div>

      <div className="grid grid-cols-4 gap-px bg-[var(--color-border)]">
        {counts.map((c) => (
          <div key={c.label} className="bg-[var(--color-bg-elevated)] px-4 py-3">
            <div className={cn("text-[18px] font-semibold", c.tone)}>{c.n}</div>
            <div className="text-[10.5px] uppercase tracking-wide text-[var(--color-fg-subtle)]">
              {c.label}
            </div>
          </div>
        ))}
      </div>

      <DiffSection
        title="Unstaged"
        scope="unstaged"
        empty={git.modified === 0 && git.conflicted === 0}
        open={openSections.unstaged}
        onToggle={() => toggle("unstaged")}
        projectPath={projectPath}
      />
      <DiffSection
        title="Staged"
        scope="staged"
        empty={git.staged === 0}
        open={openSections.staged}
        onToggle={() => toggle("staged")}
        projectPath={projectPath}
      />
      <DiffSection
        title="Untracked"
        scope="untracked"
        empty={git.untracked === 0}
        open={openSections.untracked}
        onToggle={() => toggle("untracked")}
        projectPath={projectPath}
      />
    </div>
  );
}

function DiffSection({
  title,
  scope,
  empty,
  open,
  onToggle,
  projectPath,
}: {
  title: string;
  scope: DiffScope;
  empty: boolean;
  open: boolean;
  onToggle: () => void;
  projectPath: string;
}) {
  const diff = useGitDiff(projectPath, scope, open && !empty);
  return (
    <div className="border-t border-[var(--color-border)]">
      <button
        onClick={onToggle}
        disabled={empty}
        className={cn(
          "flex w-full items-center gap-2 px-4 py-2 text-left text-[12px] font-medium text-[var(--color-fg-muted)]",
          "hover:text-[var(--color-fg)]",
          empty && "cursor-not-allowed opacity-50"
        )}
      >
        {open ? <ChevronDownIcon size={13} /> : <ChevronRightIcon size={13} />}
        {title}
        {empty && <span className="text-[var(--color-fg-subtle)]">— empty</span>}
      </button>
      {open && !empty && (
        <div className="max-h-[520px] overflow-auto border-t border-[var(--color-border)] bg-[var(--color-bg)]/40">
          {diff.isLoading && (
            <div className="px-3 py-3 text-[11px] text-[var(--color-fg-subtle)]">Loading…</div>
          )}
          {diff.error && (
            <div className="px-3 py-3 text-[11px] text-[var(--color-danger)]">
              {String(diff.error)}
            </div>
          )}
          {diff.data && <DiffView raw={diff.data} defaultOpen={scope === "unstaged"} />}
        </div>
      )}
    </div>
  );
}
