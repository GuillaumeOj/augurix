import { openUrl } from "@tauri-apps/plugin-opener";
import {
  CheckCircle2Icon,
  CircleDashedIcon,
  CircleIcon,
  ExternalLinkIcon,
  GitPullRequestDraftIcon,
  GitPullRequestIcon,
  XCircleIcon,
} from "lucide-react";
import type { PrStatus } from "@/lib/tauri";
import { cn } from "@/lib/utils";

export function PrCard({ pr }: { pr: PrStatus | null }) {
  if (!pr) {
    return (
      <div className="rounded-lg border border-dashed border-[var(--color-border)] bg-[var(--color-bg-elevated)]/40 p-4">
        <div className="flex items-center gap-2 text-[12px] text-[var(--color-fg-subtle)]">
          <GitPullRequestIcon size={13} />
          No PR for the current branch.
        </div>
      </div>
    );
  }

  const checksTone = {
    SUCCESS: "text-[var(--color-success)]",
    FAILURE: "text-[var(--color-danger)]",
    PENDING: "text-[var(--color-warning)]",
    NONE: "text-[var(--color-fg-subtle)]",
  }[pr.checks];

  const ChecksIcon = {
    SUCCESS: CheckCircle2Icon,
    FAILURE: XCircleIcon,
    PENDING: CircleDashedIcon,
    NONE: CircleIcon,
  }[pr.checks];

  const Icon = pr.is_draft ? GitPullRequestDraftIcon : GitPullRequestIcon;
  const stateTone =
    pr.state === "MERGED"
      ? "text-[var(--color-info)]"
      : pr.state === "DRAFT"
        ? "text-[var(--color-fg-muted)]"
        : pr.state === "CLOSED"
          ? "text-[var(--color-fg-muted)]"
          : "text-[var(--color-success)]";

  return (
    <button
      onClick={() => openUrl(pr.url).catch(() => {})}
      className={cn(
        "group block w-full text-left rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-4 py-3 transition-colors",
        "hover:border-[var(--color-border-strong)] hover:bg-[var(--color-surface)]/40"
      )}
    >
      <div className="flex items-center gap-3">
        <Icon size={15} className={stateTone} />
        <div className="flex-1 min-w-0">
          <div className="text-[13px] font-medium text-[var(--color-fg)] truncate">{pr.title}</div>
          <div className="text-[11px] text-[var(--color-fg-muted)]">
            #{pr.number} · {pr.state.toLowerCase()}
          </div>
        </div>
        <div className={cn("flex items-center gap-1.5 text-[12px] font-medium", checksTone)}>
          <ChecksIcon size={13} className={pr.checks === "PENDING" ? "animate-spin" : ""} />
          {pr.checks.toLowerCase()}
        </div>
        <ExternalLinkIcon
          size={13}
          className="text-[var(--color-fg-subtle)] opacity-0 group-hover:opacity-100 transition-opacity"
        />
      </div>
    </button>
  );
}
