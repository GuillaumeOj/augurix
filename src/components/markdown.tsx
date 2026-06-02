import { openUrl } from "@tauri-apps/plugin-opener";
import { createElement, memo } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { cn } from "@/lib/utils";

const REMARK_PLUGINS = [remarkGfm];

// Headings share styling and differ only by font size; the wrapper's
// `flex-col gap` handles spacing between blocks, so no per-heading margin.
const heading =
  (tag: "h1" | "h2" | "h3" | "h4", size: string): Components[typeof tag] =>
  ({ node: _node, ...props }) =>
    createElement(tag, {
      className: `${size} font-semibold text-[var(--color-fg)]`,
      ...props,
    });

const components: Components = {
  p: ({ node: _node, ...props }) => <p className="break-words" {...props} />,
  h1: heading("h1", "text-[14px]"),
  h2: heading("h2", "text-[13.5px]"),
  h3: heading("h3", "text-[13px]"),
  h4: heading("h4", "text-[12.5px]"),
  ul: ({ node: _node, ...props }) => (
    <ul className="list-disc pl-4 flex flex-col gap-0.5" {...props} />
  ),
  ol: ({ node: _node, ...props }) => (
    <ol className="list-decimal pl-4 flex flex-col gap-0.5" {...props} />
  ),
  li: ({ node: _node, ...props }) => <li className="break-words" {...props} />,
  strong: ({ node: _node, ...props }) => <strong className="font-semibold" {...props} />,
  a: ({ node: _node, href, ...props }) => (
    <a
      href={href}
      rel="noreferrer"
      className="text-[var(--color-accent)] underline underline-offset-2"
      onClick={(e) => {
        e.preventDefault();
        if (href) openUrl(href).catch(() => {});
      }}
      {...props}
    />
  ),
  code: ({ node: _node, className, ...props }) => {
    // Inline code: react-markdown renders a bare <code> (no language class) for inline
    // spans, and a <code> wrapped in <pre> for fenced blocks (styled via `pre`).
    const isBlock = className?.includes("language-");
    if (isBlock) {
      return <code className={cn("font-mono", className)} {...props} />;
    }
    return (
      <code
        className="rounded bg-[var(--color-bg)]/60 px-1 py-0.5 font-mono text-[11.5px]"
        {...props}
      />
    );
  },
  pre: ({ node: _node, ...props }) => (
    <pre
      className="overflow-x-auto rounded-md border border-[var(--color-border)] bg-[var(--color-bg)]/40 p-2 font-mono text-[11.5px] leading-snug"
      {...props}
    />
  ),
  blockquote: ({ node: _node, ...props }) => (
    <blockquote
      className="border-l-2 border-[var(--color-border)] pl-2 text-[var(--color-fg-muted)]"
      {...props}
    />
  ),
  table: ({ node: _node, ...props }) => (
    <div className="overflow-x-auto">
      <table className="border-collapse text-[11.5px]" {...props} />
    </div>
  ),
  th: ({ node: _node, ...props }) => (
    <th
      className="border border-[var(--color-border)] px-1.5 py-0.5 text-left font-semibold"
      {...props}
    />
  ),
  td: ({ node: _node, ...props }) => (
    <td className="border border-[var(--color-border)] px-1.5 py-0.5" {...props} />
  ),
  hr: ({ node: _node, ...props }) => <hr className="border-[var(--color-border)]" {...props} />,
};

// Memoized so the markdown tree is re-parsed only when `text` actually changes,
// not on every transcript refetch (the list refetches every 5s).
export const Markdown = memo(function Markdown({ text }: { text: string }) {
  return (
    <div className="mt-1 flex flex-col gap-1.5 text-[12.5px] leading-snug text-[var(--color-fg)] break-words">
      <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={components}>
        {text}
      </ReactMarkdown>
    </div>
  );
});
