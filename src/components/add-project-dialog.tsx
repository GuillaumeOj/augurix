import { useMutation, useQueryClient } from "@tanstack/react-query";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FolderPlusIcon, FolderSearchIcon, XIcon } from "lucide-react";
import { useState } from "react";
import { qk, useDiscover } from "@/lib/queries";
import { api } from "@/lib/tauri";
import { cn, relativeTime } from "@/lib/utils";

export function AddProjectDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const qc = useQueryClient();
  const discover = useDiscover(open);
  const [pending, setPending] = useState<string | null>(null);

  const add = useMutation({
    mutationFn: (path: string) => api.addProject(path),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: qk.projects });
      qc.invalidateQueries({ queryKey: qk.statusRoot });
      qc.invalidateQueries({ queryKey: qk.discover });
    },
  });

  if (!open) return null;

  async function pickFolder() {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "Select project folder",
    });
    if (typeof selected === "string") {
      setPending(selected);
      try {
        await add.mutateAsync(selected);
        onClose();
      } catch (e) {
        console.error(e);
      } finally {
        setPending(null);
      }
    }
  }

  async function addDiscovered(path: string) {
    setPending(path);
    try {
      await add.mutateAsync(path);
    } catch (e) {
      console.error(e);
    } finally {
      setPending(null);
    }
  }

  return (
    <div
      role="dialog"
      aria-modal
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="w-[640px] max-h-[80vh] flex flex-col rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] shadow-2xl"
      >
        <div className="flex items-center justify-between border-b border-[var(--color-border)] px-5 py-3.5">
          <div>
            <h2 className="text-sm font-semibold text-[var(--color-fg)]">Add a project</h2>
            <p className="text-xs text-[var(--color-fg-muted)] mt-0.5">
              Pick a folder or import a Claude Code session you already started.
            </p>
          </div>
          <button
            onClick={onClose}
            className="rounded-md p-1 text-[var(--color-fg-subtle)] hover:bg-[var(--color-surface)] hover:text-[var(--color-fg)]"
          >
            <XIcon size={16} />
          </button>
        </div>

        <div className="px-5 py-4 border-b border-[var(--color-border)]">
          <button
            onClick={pickFolder}
            className={cn(
              "w-full flex items-center gap-3 rounded-md border border-dashed border-[var(--color-border-strong)] bg-[var(--color-surface)]/40 px-4 py-3 text-left transition-colors",
              "hover:bg-[var(--color-surface)] hover:border-[var(--color-accent)]/50"
            )}
          >
            <FolderPlusIcon size={18} className="text-[var(--color-accent)]" />
            <div className="flex-1">
              <div className="text-[13px] font-medium text-[var(--color-fg)]">Choose folder…</div>
              <div className="text-[11px] text-[var(--color-fg-muted)]">
                Any directory on your disk
              </div>
            </div>
          </button>
        </div>

        <div className="flex items-center gap-2 px-5 pt-4 pb-2">
          <FolderSearchIcon size={13} className="text-[var(--color-fg-subtle)]" />
          <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--color-fg-subtle)]">
            Discovered Claude Code sessions
          </span>
          {discover.isFetching && (
            <span className="text-[11px] text-[var(--color-fg-subtle)]">scanning…</span>
          )}
        </div>

        <div className="flex-1 overflow-y-auto px-2 pb-3">
          {(!discover.data || discover.data.length === 0) && !discover.isFetching && (
            <div className="px-3 py-6 text-center text-xs text-[var(--color-fg-subtle)]">
              Nothing found in <span className="font-mono">~/.claude/projects/</span>
            </div>
          )}
          {discover.data?.map((d) => (
            <button
              key={d.path}
              disabled={d.already_added || pending === d.path}
              onClick={() => addDiscovered(d.path)}
              className={cn(
                "w-full flex items-center gap-3 rounded-md px-3 py-2 text-left transition-colors",
                "hover:bg-[var(--color-surface)]",
                d.already_added && "opacity-50 cursor-not-allowed",
                pending === d.path && "bg-[var(--color-surface)]"
              )}
            >
              <div className="flex-1 min-w-0">
                <div className="text-[13px] font-medium text-[var(--color-fg)] truncate">
                  {d.name}
                </div>
                <div className="text-[11px] font-mono text-[var(--color-fg-subtle)] truncate">
                  {d.path}
                </div>
              </div>
              <div className="text-[11px] text-[var(--color-fg-muted)] whitespace-nowrap">
                {d.already_added ? "added" : relativeTime(d.last_session_at)}
              </div>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
