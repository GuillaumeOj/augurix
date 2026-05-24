# Augurix

> *Augur* — the Roman who reads omens. Augurix reads the signs across every project you're running and tells you what matters now.

A cross-platform desktop dashboard for developers juggling many active projects at once. One window, every project at a glance: AI agent status, transcripts, git status, diff, worktrees, GitHub pull requests and checks.

## Features

- **Multi-project sidebar** — every project is a tree of *instances* (the project root, any git worktrees, and any sub-folders with active Claude Code sessions). Each instance shows its live agent state, branch, dirty/untracked counts and PR badge.
- **Auto-discovery** — instances are derived automatically from `git worktree list` plus a scan of `~/.claude/projects/` for sub-paths with recent transcripts. Transcripts launched from the project root but operating on a worktree are routed to the deepest matching instance (by `cwd`).
- **Live agent transcript** — for each instance, the last ~50 messages of the active Claude Code session: prose, tool uses (Read, Edit, Bash, Grep…), and a friendly per-tool detail (file path, command snippet, search pattern). Auto-scrolls when you're at the bottom; stays put when you've scrolled back.
- **Git panel** — branch, ahead/behind, modified / staged / untracked / conflicted counters; per-file collapsible diffs (GitHub-style) for unstaged, staged and *untracked* files (untracked content is synthesised into a real-looking diff).
- **GitHub PR card** — number, title, state, checks rollup; click opens the PR in your browser. Backed by your local `gh` CLI session (no token storage in the app).
- **Live updates** — file watcher (`notify`) + 5 s poller; status and transcript refetch on transcript appends and `.git` changes. PR results are cached for 60 s to keep network traffic down.

## Stack

| Layer | Choice |
|---|---|
| Shell | **Tauri 2.0** — ~5 MB native binary, runs on macOS, Linux, Windows |
| Frontend | **React 19 + Vite + TypeScript** |
| UI | **Tailwind CSS v4**, custom design system |
| Async state | **TanStack Query** |
| Router | **TanStack Router** (file-based, type-safe) |
| Persistence | JSON file in Tauri app-data dir |
| Git | shells out to system `git` |
| GitHub | shells out to your `gh` CLI |
| Process detection | `sysinfo` |
| File watching | `notify` + `notify-debouncer-full` |

## Requirements

- macOS / Linux / Windows
- Rust 1.77+ (for builds)
- Node 20+ / pnpm 8+
- `git` on PATH
- `gh` on PATH and authenticated (`gh auth login`) — for PR data

## Develop

```bash
pnpm install
pnpm tauri dev
```

The window opens with hot-reload for React and auto-rebuild for Rust.

## Build

```bash
pnpm tauri build --bundles app    # .app on macOS (~5 MB binary)
pnpm tauri build                  # all targets (dmg/deb/msi as platform supports)
```

## Roadmap

- Two-way agent input — send a message to a running Claude Code session from the app
- "Open in terminal" — hand off to the user's preferred terminal app in the instance's cwd
- More agents — Cursor / Codex / aider / custom
- Issue trackers — Linear, GitHub Issues, Jira
- PR hosts — GitLab, Bitbucket
- CI / deploys — surface latest workflow runs and deployment states per instance

## License

MIT
