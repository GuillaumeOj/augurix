import {
  ChevronDownIcon,
  ChevronRightIcon,
  FileMinusIcon,
  FilePlusIcon,
  FileTextIcon,
  MinusIcon,
  PlusIcon,
} from "lucide-react";
import { useState } from "react";
import { type FileDiff, parseDiff } from "@/lib/parse-diff";
import { cn } from "@/lib/utils";

export function DiffView({ raw, defaultOpen = false }: { raw: string; defaultOpen?: boolean }) {
  const files = parseDiff(raw);
  if (files.length === 0) {
    return <div className="px-3 py-3 text-[11px] text-[var(--color-fg-subtle)]">No changes.</div>;
  }
  return (
    <div className="flex flex-col gap-2 px-2 py-2">
      {files.map((file, i) => (
        <FileCard key={i} file={file} defaultOpen={defaultOpen} />
      ))}
    </div>
  );
}

function FileCard({ file, defaultOpen }: { file: FileDiff; defaultOpen: boolean }) {
  const [open, setOpen] = useState(defaultOpen);
  const displayPath = file.newPath ?? file.oldPath ?? "(unknown)";

  const Icon = file.isNewFile ? FilePlusIcon : file.isDeletedFile ? FileMinusIcon : FileTextIcon;
  const iconTone = file.isNewFile
    ? "text-[var(--color-success)]"
    : file.isDeletedFile
      ? "text-[var(--color-danger)]"
      : "text-[var(--color-fg-muted)]";

  return (
    <div className="overflow-hidden rounded-md border border-[var(--color-border)] bg-[var(--color-bg)]/30">
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center gap-2 px-2.5 py-1.5 text-left hover:bg-[var(--color-surface)]/60"
      >
        {open ? (
          <ChevronDownIcon size={12} className="text-[var(--color-fg-subtle)]" />
        ) : (
          <ChevronRightIcon size={12} className="text-[var(--color-fg-subtle)]" />
        )}
        <Icon size={12} className={iconTone} />
        <span className="flex-1 truncate font-mono text-[12px] text-[var(--color-fg)]">
          {displayPath}
        </span>
        {file.isBinary ? (
          <span className="text-[10.5px] text-[var(--color-fg-subtle)]">binary</span>
        ) : (
          <span className="flex items-center gap-2 text-[11px] font-mono">
            <span className="text-[var(--color-success)]">+{file.additions}</span>
            <span className="text-[var(--color-danger)]">−{file.deletions}</span>
          </span>
        )}
      </button>
      {open && !file.isBinary && (
        <div className="overflow-x-auto border-t border-[var(--color-border)] bg-[var(--color-bg)]/60 px-2 py-1.5 font-mono text-[11.5px] leading-[1.55]">
          {file.hunks.map((h, hi) => (
            <div key={hi} className={hi > 0 ? "mt-2" : undefined}>
              <div className="px-1 text-[11px] text-[var(--color-info)]">{h.header}</div>
              {h.lines.map((line, li) => {
                let cls = "text-[var(--color-fg-muted)]";
                let icon: React.ReactNode = <span className="inline-block w-2.5" />;
                if (line.startsWith("+") && !line.startsWith("+++")) {
                  cls = "text-[var(--color-success)] bg-[var(--color-success)]/8";
                  icon = <PlusIcon size={9} className="inline mr-1 opacity-60" />;
                } else if (line.startsWith("-") && !line.startsWith("---")) {
                  cls = "text-[var(--color-danger)] bg-[var(--color-danger)]/8";
                  icon = <MinusIcon size={9} className="inline mr-1 opacity-60" />;
                } else if (line.startsWith("\\")) {
                  cls = "text-[var(--color-fg-subtle)] italic";
                }
                return (
                  <div
                    key={li}
                    className={cn(
                      "block whitespace-pre-wrap break-words px-1 -mx-1 rounded-sm",
                      cls
                    )}
                  >
                    {icon}
                    {line || " "}
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
