import type { AgentState } from "@/lib/tauri";
import { cn } from "@/lib/utils";

const STATE_COLORS: Record<AgentState, string> = {
  running: "bg-[var(--color-success)] shadow-[0_0_10px_var(--color-success)]",
  "awaiting-input": "bg-[var(--color-warning)] shadow-[0_0_10px_var(--color-warning)]",
  idle: "bg-[var(--color-info)]/70",
  "not-running": "bg-[var(--color-fg-subtle)]/60",
};

export function StatusDot({
  state,
  size = 8,
  className,
}: {
  state: AgentState;
  size?: number;
  className?: string;
}) {
  const pulsing = state === "running" || state === "awaiting-input";
  return (
    <span
      className={cn(
        "inline-block rounded-full",
        STATE_COLORS[state],
        pulsing && "pulse-dot",
        className
      )}
      style={{ width: size, height: size }}
      role="img"
      aria-label={state}
    />
  );
}

export const STATE_LABELS: Record<AgentState, string> = {
  running: "Running",
  "awaiting-input": "Awaiting input",
  idle: "Idle",
  "not-running": "Not running",
};
