import {
  ArrowDownIcon,
  ArrowUpIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  GitBranchIcon,
  GitCompareIcon,
} from "lucide-react";
import { useState } from "react";
import {
  type DiffScope,
  useBaseDiff,
  useBranches,
  useDefaultBranch,
  useGitDiff,
  useInstanceBaseOverride,
  useSetInstanceBaseOverride,
} from "@/lib/queries";
import type { GitStatus, UUID } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { DiffView } from "./diff-view";

export function GitPanel({
  git,
  projectPath,
  instanceId,
}: {
  git: GitStatus;
  projectPath: string;
  instanceId: UUID;
}) {
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

      <BaseDiffSection projectPath={projectPath} instanceId={instanceId} />

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
        <div className="border-t border-[var(--color-border)] bg-[var(--color-bg)]/40">
          <DiffPaneBody
            isLoading={diff.isLoading}
            error={diff.error}
            data={diff.data}
            defaultOpen={scope === "unstaged"}
          />
        </div>
      )}
    </div>
  );
}

/**
 * The body of an open diff section: the loading / error / diff states. `DiffView`
 * owns its own `max-h`/scroll, so the sticky file headers it renders pin against
 * a scroll container it controls — no implicit dependency on the parent's layout.
 */
function DiffPaneBody({
  isLoading,
  error,
  data,
  defaultOpen,
}: {
  isLoading: boolean;
  error: unknown;
  data: string | undefined;
  defaultOpen: boolean;
}) {
  if (isLoading) {
    return <div className="px-3 py-3 text-[11px] text-[var(--color-fg-subtle)]">Loading…</div>;
  }
  if (error) {
    return <div className="px-3 py-3 text-[11px] text-[var(--color-danger)]">{String(error)}</div>;
  }
  if (data !== undefined) {
    return <DiffView raw={data} defaultOpen={defaultOpen} />;
  }
  return null;
}

/**
 * The whole diff of this worktree against its base branch — the delta a reviewer
 * cares about before merging. The base is auto-detected (origin/HEAD → main/…)
 * with a persisted per-instance override, and a sub-toggle switches between all
 * changes (working tree included) and committed-only (PR-style).
 */
function BaseDiffSection({ projectPath, instanceId }: { projectPath: string; instanceId: UUID }) {
  const [open, setOpen] = useState(false);
  const [includeWorkingTree, setIncludeWorkingTree] = useState(true);

  const detected = useDefaultBranch(projectPath, true);
  const override = useInstanceBaseOverride(instanceId);
  const branches = useBranches(projectPath, open);
  const setOverride = useSetInstanceBaseOverride(instanceId, projectPath);

  const effectiveBase = override.data ?? detected.data ?? undefined;
  const diff = useBaseDiff(projectPath, effectiveBase, includeWorkingTree, open && !!effectiveBase);
  const hasBase = !!effectiveBase;

  return (
    <div className="border-t border-[var(--color-border)]">
      <div className="flex items-center gap-2 px-4 py-2 text-[12px] font-medium text-[var(--color-fg-muted)]">
        <button
          onClick={() => setOpen((v) => !v)}
          className="flex items-center gap-2 text-left hover:text-[var(--color-fg)]"
        >
          {open ? <ChevronDownIcon size={13} /> : <ChevronRightIcon size={13} />}
          <GitCompareIcon size={13} />
          <span>vs {effectiveBase ?? "base"}</span>
        </button>

        {open && (
          <div className="ml-auto flex items-center gap-2">
            <select
              value={override.data ?? ""}
              onChange={(e) => setOverride.mutate(e.target.value || null)}
              className="rounded border border-[var(--color-border)] bg-[var(--color-bg)] px-1.5 py-0.5 text-[11px] text-[var(--color-fg)]"
              title="Base branch to compare against"
            >
              <option value="">Auto{detected.data ? ` (${detected.data})` : ""}</option>
              {branches.data?.map((b) => (
                <option key={b} value={b}>
                  {b}
                </option>
              ))}
            </select>
            <div className="flex overflow-hidden rounded border border-[var(--color-border)] text-[11px]">
              <SegButton
                active={includeWorkingTree}
                onClick={() => setIncludeWorkingTree(true)}
                label="All changes"
              />
              <SegButton
                active={!includeWorkingTree}
                onClick={() => setIncludeWorkingTree(false)}
                label="Committed"
              />
            </div>
          </div>
        )}
      </div>
      {open && (
        <div className="border-t border-[var(--color-border)] bg-[var(--color-bg)]/40">
          {hasBase ? (
            <DiffPaneBody
              isLoading={diff.isLoading}
              error={diff.error}
              data={diff.data}
              defaultOpen={false}
            />
          ) : (
            !detected.isLoading && (
              <div className="px-3 py-3 text-[11px] text-[var(--color-fg-subtle)]">
                No base branch detected — pick one above.
              </div>
            )
          )}
        </div>
      )}
    </div>
  );
}

function SegButton({
  active,
  onClick,
  label,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "px-2 py-0.5 transition-colors",
        active
          ? "bg-[var(--color-surface)] text-[var(--color-fg)]"
          : "text-[var(--color-fg-subtle)] hover:text-[var(--color-fg)]"
      )}
    >
      {label}
    </button>
  );
}
