import {
  SparklesIcon,
  SquareIcon,
  TerminalIcon,
  UserIcon,
  WrenchIcon,
  ZapIcon,
} from "lucide-react";
import { useEffect, useRef } from "react";
import { useInstanceMessages } from "@/lib/queries";
import type { AgentStatus, TranscriptMessage, UUID } from "@/lib/tauri";
import { cn, relativeTime } from "@/lib/utils";
import { STATE_LABELS, StatusDot } from "./status-dot";

export function AgentPanel({
  agent,
  projectId,
  instanceId,
}: {
  agent: AgentStatus;
  projectId: UUID;
  instanceId: UUID;
}) {
  const messages = useInstanceMessages(projectId, instanceId, 50);

  return (
    <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)]">
      <div className="flex items-center gap-3 border-b border-[var(--color-border)] px-4 py-3">
        <StatusDot state={agent.state} size={10} />
        <div className="flex-1 min-w-0">
          <div className="text-[13px] font-semibold text-[var(--color-fg)]">
            {STATE_LABELS[agent.state]}
          </div>
          <div className="text-[11px] text-[var(--color-fg-subtle)] truncate">
            Claude Code
            {agent.pid && ` · pid ${agent.pid}`}
            {agent.last_activity_at && ` · last activity ${relativeTime(agent.last_activity_at)}`}
            {agent.session_id && (
              <span className="font-mono ml-1">· {agent.session_id.slice(0, 8)}</span>
            )}
          </div>
        </div>
        <div className="flex items-center gap-1">
          <ActionButton
            icon={<TerminalIcon size={13} />}
            label="Open terminal"
            disabled
            tooltip="Coming soon"
          />
          <ActionButton icon={<ZapIcon size={13} />} label="Send" disabled tooltip="Coming soon" />
        </div>
      </div>

      <TranscriptList
        messages={messages.data ?? []}
        loading={messages.isLoading}
        emptyState={agent.state}
      />
    </div>
  );
}

function TranscriptList({
  messages,
  loading,
  emptyState,
}: {
  messages: TranscriptMessage[];
  loading: boolean;
  emptyState: AgentStatus["state"];
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const prevSigRef = useRef<string | null>(null);

  // Auto-scroll to bottom on new messages, but only if user is already near
  // the bottom (so they can read history without being yanked away). We use a
  // (length + last-timestamp + last-text-length) signature because timestamps
  // may be null and length alone misses in-place edits to the last entry.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const last = messages[messages.length - 1];
    const sig = last
      ? `${messages.length}|${last.timestamp ?? "?"}|${last.text?.length ?? 0}|${last.tool_uses.length}`
      : "0";
    if (sig === prevSigRef.current) return;
    prevSigRef.current = sig;
    const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
    if (nearBottom) {
      el.scrollTop = el.scrollHeight;
    }
  }, [messages]);

  if (loading && messages.length === 0) {
    return (
      <div className="px-4 py-6 text-center text-[12px] text-[var(--color-fg-subtle)]">
        Loading transcript…
      </div>
    );
  }
  if (messages.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-2 py-8 text-center">
        <SquareIcon size={18} className="text-[var(--color-fg-subtle)]" />
        <div className="text-[13px] text-[var(--color-fg-muted)]">
          {emptyState === "not-running"
            ? "No Claude Code session here yet"
            : "No transcript available"}
        </div>
        <div className="text-[11px] text-[var(--color-fg-subtle)]">
          Run <span className="font-mono">claude</span> in this instance's folder.
        </div>
      </div>
    );
  }

  return (
    <div ref={scrollRef} className="max-h-[520px] overflow-y-auto px-4 py-3 flex flex-col gap-3">
      {messages.map((m, i) => (
        <MessageRow key={`${m.timestamp}-${i}`} message={m} />
      ))}
    </div>
  );
}

function MessageRow({ message }: { message: TranscriptMessage }) {
  const isUser = message.role === "user";
  return (
    <div className="flex gap-2.5 min-w-0">
      <div
        className={cn(
          "mt-0.5 grid h-5 w-5 flex-shrink-0 place-items-center rounded-md border",
          isUser
            ? "bg-[var(--color-surface)] border-[var(--color-border)] text-[var(--color-fg-muted)]"
            : "bg-gradient-to-br from-[var(--color-accent)]/30 to-[var(--color-info)]/30 border-[var(--color-accent)]/40 text-[var(--color-accent)]"
        )}
      >
        {isUser ? <UserIcon size={11} /> : <SparklesIcon size={11} />}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-baseline gap-2">
          <span
            className={cn(
              "text-[11px] font-semibold uppercase tracking-wide",
              isUser ? "text-[var(--color-fg-muted)]" : "text-[var(--color-accent)]"
            )}
          >
            {isUser ? "You" : "Claude"}
          </span>
          <span
            className="text-[10.5px] text-[var(--color-fg-subtle)]"
            title={message.timestamp ?? undefined}
          >
            {relativeTime(message.timestamp)}
          </span>
        </div>
        {message.text && (
          <div className="mt-1 text-[12.5px] leading-snug text-[var(--color-fg)] whitespace-pre-wrap break-words">
            {message.text}
          </div>
        )}
        {message.tool_uses.length > 0 && (
          <div className="mt-1.5 flex flex-wrap gap-1">
            {message.tool_uses.map((t, i) => (
              <ToolChip key={i} name={t.name} detail={t.detail} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function ToolChip({ name, detail }: { name: string; detail: string | null }) {
  return (
    <span
      className="inline-flex max-w-full items-center gap-1 truncate rounded-md border border-[var(--color-border)] bg-[var(--color-bg)]/40 px-1.5 py-0.5 text-[10.5px] text-[var(--color-fg-muted)]"
      title={detail ?? name}
    >
      <WrenchIcon size={9} className="text-[var(--color-fg-subtle)]" />
      <span className="font-semibold text-[var(--color-fg)]">{name}</span>
      {detail && <span className="truncate font-mono text-[var(--color-fg-subtle)]">{detail}</span>}
    </span>
  );
}

function ActionButton({
  icon,
  label,
  disabled,
  tooltip,
}: {
  icon: React.ReactNode;
  label: string;
  disabled?: boolean;
  tooltip?: string;
}) {
  return (
    <button
      title={tooltip}
      disabled={disabled}
      className={cn(
        "inline-flex items-center gap-1.5 rounded-md border border-[var(--color-border)] px-2 py-1 text-[11px] font-medium",
        "text-[var(--color-fg-muted)] hover:bg-[var(--color-surface)] hover:text-[var(--color-fg)]",
        disabled && "opacity-50 cursor-not-allowed hover:bg-transparent"
      )}
    >
      {icon}
      {label}
    </button>
  );
}
