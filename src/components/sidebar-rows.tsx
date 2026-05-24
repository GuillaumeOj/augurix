import { Link, useParams, useSearch } from "@tanstack/react-router";
import {
  ChevronDownIcon,
  ChevronRightIcon,
  CornerDownRightIcon,
  FolderGit2Icon,
  GitBranchIcon,
  PinIcon,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { AgentState, InstanceWithStatus, ProjectWithInstances } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { Badge, PrBadge } from "./badges";
import { StatusDot } from "./status-dot";

function aggregateState(instances: InstanceWithStatus[]): AgentState {
  const order: AgentState[] = ["awaiting-input", "running", "idle", "not-running"];
  for (const s of order) {
    if (instances.some((i) => i.status.agent.state === s)) return s;
  }
  return "not-running";
}

function dirtyCount(g: InstanceWithStatus["status"]["git"]) {
  return g.modified + g.staged + g.untracked + g.conflicted;
}

export function ProjectGroup({ pwi }: { pwi: ProjectWithInstances }) {
  const params = useParams({ strict: false }) as { id?: string };
  const search = useSearch({ strict: false }) as { i?: string };

  const isActive = params.id === pwi.project.id;
  const activeInstanceId = isActive ? (search.i ?? pwi.instances[0]?.id) : null;

  const aggState = useMemo(() => aggregateState(pwi.instances), [pwi.instances]);
  const aggDirty = useMemo(
    () => pwi.instances.reduce((acc, i) => acc + dirtyCount(i.status.git), 0),
    [pwi.instances]
  );
  const hasOpenPr = pwi.instances.some((i) => i.status.pr && i.status.pr.state === "OPEN");

  const defaultOpen =
    isActive ||
    pwi.project.pinned ||
    aggState === "running" ||
    aggState === "awaiting-input" ||
    pwi.instances.length === 1;
  const [open, setOpen] = useState(defaultOpen);

  // Auto-open ONLY on the transition from inactive→active so the user can
  // still collapse the active group manually if they want to.
  const prevActiveRef = useRef(isActive);
  useEffect(() => {
    if (isActive && !prevActiveRef.current) {
      setOpen(true);
    }
    prevActiveRef.current = isActive;
  }, [isActive]);

  return (
    <div className="select-none">
      <div
        className={cn(
          "group flex items-center gap-1.5 rounded-md px-1.5 py-1.5 transition-colors",
          "hover:bg-[var(--color-surface)]/70",
          isActive && "bg-[var(--color-surface)]/70"
        )}
      >
        <button
          onClick={() => setOpen((v) => !v)}
          className="grid h-4 w-4 place-items-center rounded text-[var(--color-fg-subtle)] hover:text-[var(--color-fg)]"
          title={open ? "Collapse" : "Expand"}
        >
          {open ? <ChevronDownIcon size={12} /> : <ChevronRightIcon size={12} />}
        </button>
        <StatusDot state={aggState} />
        <Link
          to="/projects/$id"
          params={{ id: pwi.project.id }}
          search={{ i: undefined }}
          className="flex-1 min-w-0 flex items-center gap-1.5"
        >
          <span className="truncate text-[13px] font-medium text-[var(--color-fg)]">
            {pwi.project.name}
          </span>
          {pwi.project.pinned && <PinIcon size={10} className="text-[var(--color-fg-subtle)]" />}
          {pwi.instances.length > 1 && (
            <span className="text-[10.5px] text-[var(--color-fg-subtle)]">
              {pwi.instances.length}
            </span>
          )}
        </Link>
        {aggDirty > 0 && (
          <span
            className="text-[10px] font-mono text-[var(--color-warning)]"
            title={`${aggDirty} change${aggDirty === 1 ? "" : "s"} across instances`}
          >
            {aggDirty}
          </span>
        )}
        {hasOpenPr && <span className="h-1.5 w-1.5 rounded-full bg-[var(--color-info)]" />}
      </div>
      {open && (
        <div className="ml-3 mt-0.5 mb-1 border-l border-[var(--color-border)] pl-1">
          {pwi.instances.map((inst) => (
            <InstanceRow
              key={inst.id}
              projectId={pwi.project.id}
              inst={inst}
              isActive={isActive && activeInstanceId === inst.id}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function InstanceRow({
  projectId,
  inst,
  isActive,
}: {
  projectId: string;
  inst: InstanceWithStatus;
  isActive: boolean;
}) {
  const dirty = dirtyCount(inst.status.git);
  const Icon = inst.kind === "sub-project" ? CornerDownRightIcon : FolderGit2Icon;
  return (
    <Link
      to="/projects/$id"
      params={{ id: projectId }}
      search={{ i: inst.id }}
      className={cn(
        "group flex flex-col gap-0.5 rounded-md px-2 py-1.5 transition-colors",
        "hover:bg-[var(--color-surface)]",
        isActive && "bg-[var(--color-surface)] shadow-[inset_0_0_0_1px_var(--color-border)]"
      )}
    >
      <div className="flex items-center gap-1.5 min-w-0">
        <StatusDot state={inst.status.agent.state} size={6} />
        <Icon size={10} className="text-[var(--color-fg-subtle)]" />
        <span className="truncate text-[12px] font-mono text-[var(--color-fg-muted)]">
          {inst.label}
        </span>
      </div>
      <div className="flex items-center gap-1 flex-wrap pl-[14px]">
        {inst.status.git.is_repo && inst.status.git.branch && (
          <span className="inline-flex items-center gap-0.5 rounded-sm bg-[var(--color-bg)]/40 px-1 py-px text-[10px] font-mono text-[var(--color-fg-subtle)]">
            <GitBranchIcon size={9} />
            <span className="truncate max-w-[120px]">{inst.status.git.branch}</span>
          </span>
        )}
        {dirty > 0 && <Badge tone="warning">{dirty}</Badge>}
        {(inst.status.git.ahead > 0 || inst.status.git.behind > 0) && (
          <span className="text-[10px] font-mono text-[var(--color-fg-subtle)]">
            {inst.status.git.ahead > 0 && `↑${inst.status.git.ahead}`}
            {inst.status.git.behind > 0 && `↓${inst.status.git.behind}`}
          </span>
        )}
        {inst.status.pr && <PrBadge pr={inst.status.pr} />}
      </div>
    </Link>
  );
}
