import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { ArrowLeftIcon, CheckIcon, CircleAlertIcon, Loader2Icon, TerminalIcon } from "lucide-react";
import { api, type KittySetupResult, type TerminalChoice, type TerminalInfo } from "@/lib/tauri";
import { cn } from "@/lib/utils";

export const Route = createFileRoute("/settings")({
  component: SettingsPage,
});

const settingsKey = ["settings"] as const;
const terminalsKey = ["terminals"] as const;

function SettingsPage() {
  const qc = useQueryClient();
  const settings = useQuery({ queryKey: settingsKey, queryFn: () => api.getSettings() });
  const terminals = useQuery({ queryKey: terminalsKey, queryFn: () => api.detectTerminals() });

  const pick = useMutation({
    mutationFn: (choice: TerminalChoice) => api.setTerminal(choice),
    onSuccess: (next) => qc.setQueryData(settingsKey, next),
  });

  const selected = settings.data?.terminal ?? null;

  return (
    <div className="flex h-full flex-col">
      <header className="titlebar-drag flex flex-shrink-0 items-center gap-3 border-b border-[var(--color-border)] bg-[var(--color-bg)]/70 px-6 py-4 no-select">
        <Link
          to="/"
          className="rounded-md p-1 text-[var(--color-fg-muted)] hover:bg-[var(--color-surface)] hover:text-[var(--color-fg)]"
          title="Back"
        >
          <ArrowLeftIcon size={15} />
        </Link>
        <h1 className="text-[18px] font-semibold text-[var(--color-fg)]">Settings</h1>
      </header>

      <div className="flex-1 overflow-auto px-6 py-6">
        <div className="mx-auto max-w-2xl">
          <section>
            <h2 className="flex items-center gap-2 text-[14px] font-semibold text-[var(--color-fg)]">
              <TerminalIcon size={15} className="text-[var(--color-accent)]" />
              Terminal
            </h2>
            <p className="mt-1 text-[12.5px] leading-relaxed text-[var(--color-fg-muted)]">
              When you click <span className="font-medium">Open terminal</span> on a running
              session, Augurix jumps to the window where it's running. Pick the terminal you use.
            </p>

            <div className="mt-4 flex flex-col gap-2">
              {terminals.isLoading && (
                <div className="text-[12px] text-[var(--color-fg-subtle)]">
                  Detecting terminals…
                </div>
              )}
              {terminals.data?.map((info) => (
                <TerminalCard
                  key={info.choice}
                  info={info}
                  selected={selected === info.choice}
                  saving={pick.isPending && pick.variables === info.choice}
                  onSelect={() => pick.mutate(info.choice)}
                />
              ))}

              <UnsupportedRow label="Linux default terminal" />
              <UnsupportedRow label="Windows Terminal" />
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}

function TerminalCard({
  info,
  selected,
  saving,
  onSelect,
}: {
  info: TerminalInfo;
  selected: boolean;
  saving: boolean;
  onSelect: () => void;
}) {
  const disabled = !info.installed;
  return (
    <div
      className={cn(
        "rounded-lg border bg-[var(--color-bg-elevated)] px-4 py-3 transition-colors",
        selected ? "border-[var(--color-accent)]" : "border-[var(--color-border)]",
        disabled && "opacity-60"
      )}
    >
      <div className="flex items-center gap-3">
        <button
          type="button"
          disabled={disabled || saving}
          onClick={onSelect}
          className={cn(
            "grid h-5 w-5 flex-shrink-0 place-items-center rounded-full border",
            selected
              ? "border-[var(--color-accent)] bg-[var(--color-accent)] text-[var(--color-accent-fg)]"
              : "border-[var(--color-border-strong)]",
            !disabled && "cursor-pointer"
          )}
          title={disabled ? "Not installed" : `Use ${info.name}`}
        >
          {saving ? (
            <Loader2Icon size={11} className="animate-spin" />
          ) : selected ? (
            <CheckIcon size={12} strokeWidth={3} />
          ) : null}
        </button>

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-[13px] font-medium text-[var(--color-fg)]">{info.name}</span>
            {selected && (
              <span className="rounded bg-[var(--color-accent)]/15 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-[var(--color-accent)]">
                Selected
              </span>
            )}
          </div>
          {info.note && (
            <div className="mt-0.5 flex items-center gap-1 text-[11.5px] text-[var(--color-fg-subtle)]">
              {!info.ready && info.installed && (
                <CircleAlertIcon size={11} className="text-[var(--color-warning,#d9a441)]" />
              )}
              {info.note}
            </div>
          )}
        </div>
      </div>

      {info.choice === "kitty" && info.installed && !info.ready && <KittySetup />}
    </div>
  );
}

function KittySetup() {
  const qc = useQueryClient();
  const setup = useMutation<KittySetupResult>({
    mutationFn: () => api.setupKitty(),
    onSuccess: () => qc.invalidateQueries({ queryKey: terminalsKey }),
  });

  return (
    <div className="mt-3 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)]/40 px-3 py-2.5">
      <p className="text-[11.5px] leading-relaxed text-[var(--color-fg-muted)]">
        kitty needs remote control enabled so Augurix can focus the right window. This appends two
        lines to <span className="font-mono">~/.config/kitty/kitty.conf</span> (your existing config
        is backed up first).
      </p>
      <button
        type="button"
        disabled={setup.isPending}
        onClick={() => setup.mutate()}
        className="mt-2 inline-flex items-center gap-1.5 rounded-md border border-[var(--color-border-strong)] bg-[var(--color-surface)] px-2.5 py-1 text-[11.5px] font-medium text-[var(--color-fg)] hover:bg-[var(--color-bg-elevated)] disabled:opacity-60"
      >
        {setup.isPending && <Loader2Icon size={12} className="animate-spin" />}
        Set up automatically
      </button>

      {setup.data && (
        <div className="mt-2 flex items-start gap-1.5 text-[11.5px] text-[var(--color-fg-muted)]">
          <CheckIcon size={12} className="mt-0.5 flex-shrink-0 text-[var(--color-accent)]" />
          <span>{setup.data.message}</span>
        </div>
      )}
      {setup.error && (
        <div className="mt-2 text-[11.5px] text-[var(--color-danger)]">{String(setup.error)}</div>
      )}
    </div>
  );
}

function UnsupportedRow({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-3 rounded-lg border border-dashed border-[var(--color-border)] px-4 py-3 opacity-50">
      <div className="h-5 w-5 flex-shrink-0 rounded-full border border-[var(--color-border-strong)]" />
      <span className="text-[13px] text-[var(--color-fg-muted)]">{label}</span>
      <span className="ml-auto text-[11px] text-[var(--color-fg-subtle)]">Not yet supported</span>
    </div>
  );
}
