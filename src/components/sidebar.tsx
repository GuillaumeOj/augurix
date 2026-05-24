import { PlusIcon, SparklesIcon } from "lucide-react";
import { useState } from "react";
import { useAllStatuses } from "@/lib/queries";
import { AddProjectDialog } from "./add-project-dialog";
import { ProjectGroup } from "./sidebar-rows";

export function Sidebar() {
  const statuses = useAllStatuses();
  const [addOpen, setAddOpen] = useState(false);

  const totalInstances = statuses.data?.reduce((acc, p) => acc + p.instances.length, 0);

  return (
    <aside className="flex h-full w-[300px] flex-col border-r border-[var(--color-border)] bg-[var(--color-bg)]/70">
      <div className="titlebar-drag flex items-center justify-between gap-2 border-b border-[var(--color-border)] px-3 py-2.5 no-select">
        <div className="flex items-center gap-2">
          <div className="grid h-6 w-6 place-items-center rounded-md bg-gradient-to-br from-[var(--color-accent)] to-[var(--color-info)] text-[var(--color-accent-fg)]">
            <SparklesIcon size={13} strokeWidth={2.5} />
          </div>
          <span className="text-[13px] font-semibold tracking-tight text-[var(--color-fg)]">
            Augurix
          </span>
          {statuses.data && (
            <span className="text-[11px] text-[var(--color-fg-subtle)]">
              {statuses.data.length}
              {typeof totalInstances === "number" &&
                totalInstances !== statuses.data.length &&
                ` · ${totalInstances}`}
            </span>
          )}
        </div>
        <button
          onClick={() => setAddOpen(true)}
          className="rounded-md p-1 text-[var(--color-fg-muted)] hover:bg-[var(--color-surface)] hover:text-[var(--color-fg)]"
          title="Add project"
        >
          <PlusIcon size={15} />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-1.5 py-1.5">
        {statuses.isLoading && (
          <div className="px-3 py-4 text-xs text-[var(--color-fg-subtle)]">Loading…</div>
        )}
        {statuses.data?.length === 0 && !statuses.isLoading && (
          <div className="px-3 py-6 text-xs text-[var(--color-fg-subtle)]">
            No projects yet. Click <span className="text-[var(--color-accent)]">+</span> to add one.
          </div>
        )}
        <div className="flex flex-col">
          {statuses.data?.map((pwi) => (
            <ProjectGroup key={pwi.project.id} pwi={pwi} />
          ))}
        </div>
      </div>

      <AddProjectDialog open={addOpen} onClose={() => setAddOpen(false)} />
    </aside>
  );
}
