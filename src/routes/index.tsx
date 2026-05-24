import { createFileRoute } from "@tanstack/react-router";
import { FolderPlusIcon, SparklesIcon } from "lucide-react";
import { useProjects } from "@/lib/queries";

export const Route = createFileRoute("/")({
  component: IndexPage,
});

function IndexPage() {
  const projects = useProjects();
  const count = projects.data?.length ?? 0;

  return (
    <div className="flex h-full items-center justify-center p-10">
      <div className="max-w-md text-center">
        <div className="mx-auto mb-5 grid h-14 w-14 place-items-center rounded-2xl bg-gradient-to-br from-[var(--color-accent)] to-[var(--color-info)] text-[var(--color-accent-fg)] shadow-lg">
          <SparklesIcon size={26} strokeWidth={2.2} />
        </div>
        <h1 className="text-xl font-semibold text-[var(--color-fg)] mb-2">
          {count === 0 ? "Welcome to Augurix" : "Pick a project from the sidebar"}
        </h1>
        <p className="text-sm text-[var(--color-fg-muted)] leading-relaxed mb-6">
          {count === 0
            ? "Track Claude Code sessions, git status, and PRs across all your projects from one dashboard."
            : `${count} project${count === 1 ? "" : "s"} tracked. Click one to see git diff, agent status, and PR checks.`}
        </p>
        {count === 0 && (
          <div className="flex items-center justify-center gap-1.5 rounded-md border border-dashed border-[var(--color-border-strong)] bg-[var(--color-bg-elevated)]/40 px-4 py-2.5 text-[12px] text-[var(--color-fg-muted)]">
            <FolderPlusIcon size={13} className="text-[var(--color-accent)]" />
            Click <span className="text-[var(--color-accent)] font-semibold">+</span> in the sidebar
            to add your first project
          </div>
        )}
      </div>
    </div>
  );
}
