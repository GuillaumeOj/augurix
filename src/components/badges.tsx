import {
  CheckCircle2Icon,
  CircleDashedIcon,
  CircleIcon,
  GitBranchIcon,
  GitPullRequestDraftIcon,
  GitPullRequestIcon,
  XCircleIcon,
} from "lucide-react";
import type { ChecksRollup, GitStatus, PrState, PrStatus } from "@/lib/tauri";
import { cn } from "@/lib/utils";

export function Badge({
  children,
  tone = "neutral",
  className,
}: {
  children: React.ReactNode;
  tone?: "neutral" | "success" | "warning" | "danger" | "info" | "accent";
  className?: string;
}) {
  const tones = {
    neutral: "bg-[var(--color-surface)] text-[var(--color-fg-muted)] border-[var(--color-border)]",
    success:
      "bg-[var(--color-success)]/12 text-[var(--color-success)] border-[var(--color-success)]/30",
    warning:
      "bg-[var(--color-warning)]/12 text-[var(--color-warning)] border-[var(--color-warning)]/30",
    danger:
      "bg-[var(--color-danger)]/15 text-[var(--color-danger)] border-[var(--color-danger)]/30",
    info: "bg-[var(--color-info)]/12 text-[var(--color-info)] border-[var(--color-info)]/30",
    accent:
      "bg-[var(--color-accent)]/12 text-[var(--color-accent)] border-[var(--color-accent)]/30",
  } as const;
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-[11px] font-medium leading-none",
        tones[tone],
        className
      )}
    >
      {children}
    </span>
  );
}

export function GitBadge({ git }: { git: GitStatus }) {
  if (!git.is_repo) {
    return <Badge tone="neutral">no git</Badge>;
  }
  const dirty = git.modified + git.staged + git.untracked + git.conflicted > 0;
  return (
    <Badge tone={dirty ? "warning" : "neutral"}>
      <GitBranchIcon size={11} />
      <span className="font-mono">{git.branch || "detached"}</span>
      {dirty && (
        <span className="opacity-80">
          {" "}
          · {git.modified + git.untracked + git.conflicted + git.staged}
        </span>
      )}
    </Badge>
  );
}

function checksIcon(r: ChecksRollup) {
  switch (r) {
    case "SUCCESS":
      return <CheckCircle2Icon size={11} />;
    case "FAILURE":
      return <XCircleIcon size={11} />;
    case "PENDING":
      return <CircleDashedIcon size={11} className="animate-spin-slow" />;
    case "NONE":
      return <CircleIcon size={11} />;
  }
}

function prToneFor(state: PrState, checks: ChecksRollup) {
  if (state === "MERGED") return "info" as const;
  if (state === "CLOSED") return "neutral" as const;
  if (state === "DRAFT") return "neutral" as const;
  if (checks === "FAILURE") return "danger" as const;
  if (checks === "PENDING") return "warning" as const;
  return "success" as const;
}

export function PrBadge({ pr }: { pr: PrStatus }) {
  const tone = prToneFor(pr.state, pr.checks);
  const Icon = pr.is_draft ? GitPullRequestDraftIcon : GitPullRequestIcon;
  return (
    <Badge tone={tone}>
      <Icon size={11} />#{pr.number} {checksIcon(pr.checks)}
    </Badge>
  );
}
