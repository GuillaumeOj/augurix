import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  createFileRoute,
  Link,
  notFound,
  useNavigate,
  useParams,
  useSearch,
} from "@tanstack/react-router";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  CornerDownRightIcon,
  FolderGit2Icon,
  FolderIcon,
  GitBranchIcon,
  GitForkIcon,
  PinIcon,
  PinOffIcon,
  RefreshCwIcon,
  Trash2Icon,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { AgentPanel } from "@/components/agent-panel";
import { GitPanel } from "@/components/git-panel";
import { PrCard } from "@/components/pr-card";
import { StatusDot } from "@/components/status-dot";
import { qk, useAllStatuses, useInstanceStatus } from "@/lib/queries";
import { api, type InstanceKind } from "@/lib/tauri";
import { cn, relativeTime } from "@/lib/utils";

export const Route = createFileRoute("/projects/$id")({
  validateSearch: (search: Record<string, unknown>) => ({
    i: typeof search.i === "string" ? search.i : undefined,
  }),
  component: ProjectDetail,
});

function ProjectDetail() {
  const { id } = useParams({ from: "/projects/$id" });
  const search = useSearch({ from: "/projects/$id" });
  const navigate = useNavigate();
  const all = useAllStatuses();
  const qc = useQueryClient();
  const [confirming, setConfirming] = useState(false);

  const pwi = all.data?.find((p) => p.project.id === id);
  const defaultInstanceId = pwi?.instances[0]?.id;
  // True iff the URL's ?i= points at an instance that no longer exists in the
  // refreshed project (e.g. worktree removed externally).
  const requestedMissing = !!pwi && !!search.i && !pwi.instances.some((i) => i.id === search.i);

  // Auto-select the first instance when either no ?i= is present OR the
  // requested ?i= no longer exists.
  useEffect(() => {
    if ((!search.i || requestedMissing) && defaultInstanceId) {
      navigate({
        to: "/projects/$id",
        params: { id },
        search: { i: defaultInstanceId },
        replace: true,
      });
    }
  }, [search.i, requestedMissing, defaultInstanceId, id, navigate]);

  const activeId =
    pwi && search.i && pwi.instances.some((i) => i.id === search.i) ? search.i : defaultInstanceId;
  const fresh = useInstanceStatus(id, activeId);

  // Manual-only spinner: ignore background refetches, only spin on user click.
  const [manualSpin, setManualSpin] = useState(false);
  useEffect(() => {
    if (manualSpin && !fresh.isFetching) {
      const t = setTimeout(() => setManualSpin(false), 200);
      return () => clearTimeout(t);
    }
  }, [manualSpin, fresh.isFetching]);
  const triggerRefresh = () => {
    setManualSpin(true);
    fresh.refetch();
  };

  // Prefer the live single-instance refetch; fall back to bundled list
  const activeInstance = useMemo(() => {
    if (fresh.data) return fresh.data;
    return pwi?.instances.find((i) => i.id === activeId);
  }, [fresh.data, pwi, activeId]);

  const removeMut = useMutation({
    mutationFn: () => api.removeProject(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: qk.projects });
      qc.invalidateQueries({ queryKey: qk.statusRoot });
      navigate({ to: "/" });
    },
  });

  const pinMut = useMutation({
    mutationFn: () => api.setPinned(id, !(pwi?.project.pinned ?? false)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: qk.projects });
      qc.invalidateQueries({ queryKey: qk.statusRoot });
    },
  });

  if (!pwi) {
    if (!all.isLoading) throw notFound();
    return <div className="p-6 text-sm text-[var(--color-fg-muted)]">Loading…</div>;
  }

  return (
    <div className="flex h-full flex-col">
      <header className="titlebar-drag flex flex-shrink-0 items-start justify-between gap-4 border-b border-[var(--color-border)] bg-[var(--color-bg)]/70 px-6 py-4 no-select">
        <div className="min-w-0">
          <h1 className="text-[18px] font-semibold text-[var(--color-fg)] truncate">
            {pwi.project.name}
          </h1>
          <button
            onClick={() => openPath(pwi.project.path).catch(() => {})}
            className="mt-0.5 flex items-center gap-1.5 text-[11.5px] font-mono text-[var(--color-fg-subtle)] hover:text-[var(--color-fg-muted)] transition-colors"
            title="Open folder"
          >
            <FolderIcon size={11} />
            {pwi.project.path}
          </button>
        </div>
        <div className="flex items-center gap-1">
          <span className="mr-2 text-[11px] text-[var(--color-fg-subtle)]">
            {activeInstance && `updated ${relativeTime(activeInstance.status.last_refreshed)}`}
          </span>
          <IconButton title="Refresh" onClick={triggerRefresh} spinning={manualSpin}>
            <RefreshCwIcon size={13} />
          </IconButton>
          <IconButton title={pwi.project.pinned ? "Unpin" : "Pin"} onClick={() => pinMut.mutate()}>
            {pwi.project.pinned ? <PinOffIcon size={13} /> : <PinIcon size={13} />}
          </IconButton>
          <IconButton
            title="Remove project"
            tone="danger"
            onClick={() => {
              if (confirming) removeMut.mutate();
              else {
                setConfirming(true);
                setTimeout(() => setConfirming(false), 2500);
              }
            }}
          >
            <Trash2Icon size={13} />
            {confirming && <span className="text-[11px] ml-1">Confirm</span>}
          </IconButton>
        </div>
      </header>

      {pwi.instances.length > 1 && (
        <div className="flex flex-shrink-0 items-center gap-1 overflow-x-auto border-b border-[var(--color-border)] bg-[var(--color-bg)]/40 px-3 py-2">
          {pwi.instances.map((inst) => (
            <Link
              key={inst.id}
              to="/projects/$id"
              params={{ id }}
              search={{ i: inst.id }}
              className={cn(
                "group inline-flex items-center gap-2 rounded-md border border-transparent px-2.5 py-1.5 text-[12px] transition-colors whitespace-nowrap",
                "hover:bg-[var(--color-surface)]",
                activeId === inst.id && "bg-[var(--color-surface)] border-[var(--color-border)]"
              )}
            >
              <StatusDot state={inst.status.agent.state} size={7} />
              <InstanceKindIcon kind={inst.kind} />
              <span className="font-mono text-[var(--color-fg-muted)]">{inst.label}</span>
              {inst.status.git.branch && (
                <span className="inline-flex items-center gap-0.5 text-[10.5px] text-[var(--color-fg-subtle)]">
                  <GitBranchIcon size={9} />
                  {inst.status.git.branch}
                </span>
              )}
            </Link>
          ))}
        </div>
      )}

      <div className="flex-1 overflow-auto px-6 py-5">
        {fresh.error && (
          <div className="rounded-lg border border-[var(--color-danger)]/40 bg-[var(--color-danger)]/10 px-4 py-3 text-sm text-[var(--color-danger)] mb-4">
            {String(fresh.error)}
          </div>
        )}

        {activeInstance && (
          <div className="grid gap-4">
            <div className="flex items-center gap-2 text-[11px] text-[var(--color-fg-subtle)]">
              <InstanceKindIcon kind={activeInstance.kind} />
              <span className="font-mono">{activeInstance.path}</span>
            </div>
            <AgentPanel
              agent={activeInstance.status.agent}
              projectId={id}
              instanceId={activeInstance.id}
            />
            <PrCard pr={activeInstance.status.pr ?? null} />
            <GitPanel git={activeInstance.status.git} projectPath={activeInstance.path} />
          </div>
        )}
      </div>
    </div>
  );
}

function InstanceKindIcon({ kind }: { kind: InstanceKind }) {
  if (kind === "linked-worktree") return <GitForkIcon size={11} />;
  if (kind === "sub-project") return <CornerDownRightIcon size={11} />;
  return <FolderGit2Icon size={11} />;
}

function IconButton({
  children,
  onClick,
  title,
  tone = "neutral",
  spinning,
}: {
  children: React.ReactNode;
  onClick?: () => void;
  title?: string;
  tone?: "neutral" | "danger";
  spinning?: boolean;
}) {
  return (
    <button
      title={title}
      onClick={onClick}
      className={cn(
        "inline-flex items-center gap-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-2 py-1 text-[var(--color-fg-muted)] transition-colors",
        "hover:bg-[var(--color-surface)] hover:text-[var(--color-fg)]",
        tone === "danger" &&
          "hover:border-[var(--color-danger)]/40 hover:text-[var(--color-danger)]"
      )}
    >
      <span className={cn(spinning && "animate-spin")}>{children}</span>
    </button>
  );
}
