import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  Loader2Icon,
  SendIcon,
  SparklesIcon,
  SquareIcon,
  TerminalIcon,
  UserIcon,
  WrenchIcon,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { qk, useInstanceMessages } from "@/lib/queries";
import {
  type AgentStatus,
  api,
  type PendingInteraction,
  type PendingQuestion,
  type SessionInput,
  type TranscriptMessage,
  type UUID,
} from "@/lib/tauri";
import { cn, relativeTime } from "@/lib/utils";
import { Markdown } from "./markdown";
import { STATE_LABELS, StatusDot } from "./status-dot";

/** Sends input into the running session; resolves `true` on success. */
type SendInput = (input: SessionInput) => Promise<boolean>;

export function AgentPanel({
  agent,
  projectId,
  instanceId,
  cwd,
  projectRoot,
}: {
  agent: AgentStatus;
  projectId: UUID;
  instanceId: UUID;
  cwd: string;
  projectRoot: string;
}) {
  const messages = useInstanceMessages(projectId, instanceId, 50);
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const [sendError, setSendError] = useState<string | null>(null);

  const openTerminal = async () => {
    if (opening) return;
    setOpening(true);
    setError(null);
    try {
      const outcome = await api.openTerminal(agent.pid, cwd, projectRoot);
      switch (outcome.status) {
        case "needs-setup":
        case "kitty-needs-setup":
          navigate({ to: "/settings" });
          break;
        case "error":
          setError(outcome.message);
          break;
        // "ok": focus already jumped — nothing to do.
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setOpening(false);
    }
  };

  const sendInput: SendInput = async (input) => {
    if (sending) return false;
    setSending(true);
    setSendError(null);
    try {
      await api.sendSessionInput(agent.pid, cwd, projectRoot, input);
      // The TUI advances asynchronously; refetch so the answered prompt clears.
      queryClient.invalidateQueries({ queryKey: qk.messages(projectId, instanceId) });
      return true;
    } catch (e) {
      setSendError(String(e));
      return false;
    } finally {
      setSending(false);
    }
  };

  const list = messages.data ?? [];
  const hasPending = list.some((m) => m.tool_uses.some((t) => t.pending));
  const canSend = agent.state !== "not-running";

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
            icon={
              opening ? (
                <Loader2Icon size={13} className="animate-spin" />
              ) : (
                <TerminalIcon size={13} />
              )
            }
            label="Open terminal"
            onClick={openTerminal}
            disabled={opening}
            tooltip="Jump to the terminal window running this session"
          />
        </div>
      </div>

      {(error || sendError) && (
        <div className="border-b border-[var(--color-border)] bg-[var(--color-danger)]/10 px-4 py-2 text-[11.5px] text-[var(--color-danger)]">
          {error ?? sendError}
        </div>
      )}

      <TranscriptList
        messages={list}
        loading={messages.isLoading}
        emptyState={agent.state}
        sendInput={sendInput}
        sending={sending}
      />

      {agent.state === "running" && !hasPending && (
        <div className="flex items-center gap-2 border-t border-[var(--color-border)] px-4 py-2.5 text-[12px] text-[var(--color-fg-muted)]">
          <Loader2Icon size={13} className="animate-spin text-[var(--color-accent)]" />
          Claude is working…
        </div>
      )}

      <ReplyComposer
        disabled={!canSend}
        sending={sending}
        onSend={(text) => sendInput({ kind: "text", text })}
      />
    </div>
  );
}

function TranscriptList({
  messages,
  loading,
  emptyState,
  sendInput,
  sending,
}: {
  messages: TranscriptMessage[];
  loading: boolean;
  emptyState: AgentStatus["state"];
  sendInput: SendInput;
  sending: boolean;
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
        <MessageRow
          key={`${m.timestamp}-${i}`}
          message={m}
          sendInput={sendInput}
          sending={sending}
        />
      ))}
    </div>
  );
}

function MessageRow({
  message,
  sendInput,
  sending,
}: {
  message: TranscriptMessage;
  sendInput: SendInput;
  sending: boolean;
}) {
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
        {message.text && <Markdown text={message.text} />}
        {message.tool_uses.length > 0 && (
          <div className="mt-1.5 flex flex-col gap-1.5">
            {message.tool_uses.map((t, i) =>
              t.pending ? (
                <PendingCard key={i} pending={t.pending} sendInput={sendInput} sending={sending} />
              ) : null
            )}
            <div className="flex flex-wrap gap-1">
              {message.tool_uses.map((t, i) =>
                t.pending ? null : <ToolChip key={i} name={t.name} detail={t.detail} />
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function PendingCard({
  pending,
  sendInput,
  sending,
}: {
  pending: PendingInteraction;
  sendInput: SendInput;
  sending: boolean;
}) {
  if (pending.kind === "plan-approval") {
    return (
      <div className="rounded-lg border border-[var(--color-accent)]/40 bg-[var(--color-accent)]/5 p-3">
        <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-[var(--color-accent)]">
          Plan — awaiting approval
        </div>
        <Markdown text={pending.plan} />
        <div className="mt-2.5 flex gap-2">
          <CardButton
            primary
            disabled={sending}
            onClick={() => sendInput({ kind: "plan", approve: true })}
          >
            Approve
          </CardButton>
          <CardButton
            disabled={sending}
            onClick={() => sendInput({ kind: "plan", approve: false })}
          >
            Reject
          </CardButton>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3 rounded-lg border border-[var(--color-accent)]/40 bg-[var(--color-accent)]/5 p-3">
      {pending.questions.map((q, i) => (
        <QuestionBlock key={i} question={q} sendInput={sendInput} sending={sending} />
      ))}
    </div>
  );
}

function QuestionBlock({
  question,
  sendInput,
  sending,
}: {
  question: PendingQuestion;
  sendInput: SendInput;
  sending: boolean;
}) {
  const [selected, setSelected] = useState<number[]>([]);

  const header = (
    <div>
      {question.header && (
        <div className="text-[11px] font-semibold uppercase tracking-wide text-[var(--color-accent)]">
          {question.header}
        </div>
      )}
      <div className="text-[13px] font-medium text-[var(--color-fg)]">{question.question}</div>
    </div>
  );

  if (question.multi_select) {
    const toggle = (i: number) =>
      setSelected((s) => (s.includes(i) ? s.filter((x) => x !== i) : [...s, i]));
    return (
      <div className="flex flex-col gap-1.5">
        {header}
        {question.options.map((o, i) => (
          <button
            type="button"
            key={i}
            disabled={sending}
            onClick={() => toggle(i)}
            className={cn(
              "flex items-start gap-2 rounded-md border px-2.5 py-1.5 text-left",
              selected.includes(i)
                ? "border-[var(--color-accent)] bg-[var(--color-accent)]/10"
                : "border-[var(--color-border)] hover:bg-[var(--color-surface)]",
              sending && "opacity-50 cursor-not-allowed"
            )}
          >
            <span
              className={cn(
                "mt-0.5 grid h-3.5 w-3.5 flex-shrink-0 place-items-center rounded border text-[9px]",
                selected.includes(i)
                  ? "border-[var(--color-accent)] bg-[var(--color-accent)] text-white"
                  : "border-[var(--color-border)]"
              )}
            >
              {selected.includes(i) ? "✓" : ""}
            </span>
            <OptionLabel option={o} />
          </button>
        ))}
        <div className="mt-0.5">
          <CardButton
            primary
            disabled={sending || selected.length === 0}
            onClick={async () => {
              const ok = await sendInput({
                kind: "option",
                indices: selected,
                multi_select: true,
              });
              if (ok) setSelected([]);
            }}
          >
            Confirm {selected.length > 0 ? `(${selected.length})` : ""}
          </CardButton>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1.5">
      {header}
      {question.options.map((o, i) => (
        <button
          type="button"
          key={i}
          disabled={sending}
          onClick={() => sendInput({ kind: "option", indices: [i], multi_select: false })}
          className={cn(
            "flex items-start gap-2 rounded-md border border-[var(--color-border)] px-2.5 py-1.5 text-left",
            "hover:border-[var(--color-accent)] hover:bg-[var(--color-accent)]/10",
            sending && "opacity-50 cursor-not-allowed"
          )}
        >
          <OptionLabel option={o} />
        </button>
      ))}
    </div>
  );
}

function OptionLabel({ option }: { option: PendingQuestion["options"][number] }) {
  return (
    <span className="min-w-0">
      <span className="block text-[12.5px] font-semibold text-[var(--color-fg)]">
        {option.label}
      </span>
      {option.description && (
        <span className="block text-[11px] text-[var(--color-fg-subtle)]">
          {option.description}
        </span>
      )}
    </span>
  );
}

function ReplyComposer({
  disabled,
  sending,
  onSend,
}: {
  disabled: boolean;
  sending: boolean;
  onSend: (text: string) => Promise<boolean>;
}) {
  const [text, setText] = useState("");

  const submit = async () => {
    const t = text.trim();
    if (!t || disabled || sending) return;
    const ok = await onSend(t);
    if (ok) setText("");
  };

  return (
    <div className="flex items-end gap-2 border-t border-[var(--color-border)] px-3 py-3">
      <textarea
        value={text}
        disabled={disabled || sending}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
            e.preventDefault();
            void submit();
          }
        }}
        rows={1}
        placeholder={disabled ? "No running session to reply to" : "Reply to Claude… (⌘↵ to send)"}
        className={cn(
          "flex-1 resize-none rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 text-[12.5px] text-[var(--color-fg)]",
          "placeholder:text-[var(--color-fg-subtle)] focus:border-[var(--color-accent)] focus:outline-none",
          (disabled || sending) && "opacity-50"
        )}
      />
      <CardButton
        primary
        disabled={disabled || sending || !text.trim()}
        onClick={() => void submit()}
      >
        {sending ? <Loader2Icon size={13} className="animate-spin" /> : <SendIcon size={13} />}
        Send
      </CardButton>
    </div>
  );
}

function CardButton({
  children,
  onClick,
  disabled,
  primary,
}: {
  children: React.ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  primary?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "inline-flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[11.5px] font-medium",
        primary
          ? "bg-[var(--color-accent)] text-white hover:opacity-90"
          : "border border-[var(--color-border)] text-[var(--color-fg-muted)] hover:bg-[var(--color-surface)] hover:text-[var(--color-fg)]",
        disabled && "opacity-50 cursor-not-allowed hover:opacity-50"
      )}
    >
      {children}
    </button>
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
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  disabled?: boolean;
  tooltip?: string;
  onClick?: () => void;
}) {
  return (
    <button
      title={tooltip}
      onClick={onClick}
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
