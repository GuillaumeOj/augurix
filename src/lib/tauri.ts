import { invoke } from "@tauri-apps/api/core";

export type UUID = string;
export type IsoDate = string;

export interface Project {
  id: UUID;
  name: string;
  path: string;
  added_at: IsoDate;
  pinned: boolean;
}

export type AgentKind = "claude-code";
export type AgentState = "not-running" | "idle" | "running" | "awaiting-input";

export interface AgentStatus {
  kind: AgentKind;
  state: AgentState;
  pid: number | null;
  session_id: string | null;
  last_message_preview: string | null;
  last_activity_at: IsoDate | null;
}

export interface GitStatus {
  is_repo: boolean;
  branch: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  modified: number;
  staged: number;
  untracked: number;
  conflicted: number;
}

export type PrState = "OPEN" | "CLOSED" | "MERGED" | "DRAFT";
export type ChecksRollup = "PENDING" | "SUCCESS" | "FAILURE" | "NONE";

export interface PrStatus {
  number: number;
  title: string;
  state: PrState;
  checks: ChecksRollup;
  url: string;
  is_draft: boolean;
}

export type InstanceKind = "main-worktree" | "linked-worktree" | "sub-project";

export interface Instance {
  id: UUID;
  project_id: UUID;
  kind: InstanceKind;
  path: string;
  label: string;
  branch_hint: string | null;
}

export interface InstanceStatus {
  instance_id: UUID;
  agent: AgentStatus;
  git: GitStatus;
  pr: PrStatus | null;
  last_refreshed: IsoDate;
}

export interface InstanceWithStatus extends Instance {
  status: InstanceStatus;
}

export interface ProjectWithInstances {
  project: Project;
  instances: InstanceWithStatus[];
}

export type MessageRole = "user" | "assistant";

export interface QuestionOption {
  label: string;
  description: string | null;
}

export interface PendingQuestion {
  header: string | null;
  question: string;
  multi_select: boolean;
  options: QuestionOption[];
}

/** An interactive prompt from Claude Code awaiting the user's answer. */
export type PendingInteraction =
  | { kind: "question"; questions: PendingQuestion[] }
  | { kind: "plan-approval"; plan: string };

export interface ToolUseEntry {
  name: string;
  detail: string | null;
  /** Set only for interactive tools we may answer. */
  tool_use_id?: string | null;
  /** Present only while this tool_use is an unanswered interactive prompt. */
  pending?: PendingInteraction | null;
}

export interface ScreenPromptOption {
  number: number;
  label: string;
  /** True for the option the TUI cursor (❯) currently sits on. */
  selected: boolean;
}

/** A selection prompt scraped from the session's live terminal screen — e.g. a
 * permission request or plan-approval prompt that never reaches the transcript. */
export interface ScreenPrompt {
  title: string | null;
  options: ScreenPromptOption[];
}

/** What to send into a running Claude Code session. Mirrors the Rust enum. */
export type SessionInput =
  | { kind: "text"; text: string }
  | { kind: "option"; indices: number[]; multi_select: boolean }
  | { kind: "plan"; approve: boolean }
  | { kind: "screen-choice"; number: number };

export interface TranscriptMessage {
  /** `null` if the source entry had no timestamp; never fabricated. */
  timestamp: IsoDate | null;
  role: MessageRole;
  text: string | null;
  tool_uses: ToolUseEntry[];
}

export interface DiscoveredProject {
  path: string;
  name: string;
  last_session_at: IsoDate | null;
  already_added: boolean;
}

export type TerminalChoice = "apple-terminal" | "iterm2" | "kitty";

export interface Settings {
  terminal: TerminalChoice | null;
}

export interface TerminalInfo {
  choice: TerminalChoice;
  name: string;
  installed: boolean;
  ready: boolean;
  note: string | null;
}

export type OpenTerminalOutcome =
  | { status: "ok" }
  | { status: "needs-setup" }
  | { status: "kitty-needs-setup" }
  | { status: "error"; message: string };

export interface KittySetupResult {
  changed: boolean;
  config_path: string;
  backup_path: string | null;
  needs_restart: boolean;
  ready: boolean;
  message: string;
}

export const api = {
  listProjects: () => invoke<Project[]>("list_projects"),
  addProject: (path: string) => invoke<Project>("add_project", { path }),
  removeProject: (id: UUID) => invoke<void>("remove_project", { id }),
  renameProject: (id: UUID, name: string) => invoke<Project | null>("rename_project", { id, name }),
  setPinned: (id: UUID, pinned: boolean) => invoke<void>("set_pinned", { id, pinned }),

  gitDiff: (path: string, scope: "unstaged" | "staged" | "untracked") =>
    invoke<string>("git_diff", { path, scope }),
  gitDefaultBranch: (path: string) => invoke<string | null>("git_default_branch", { path }),
  gitBranches: (path: string) => invoke<string[]>("git_branches", { path }),
  gitBaseDiff: (path: string, base: string, includeWorkingTree: boolean) =>
    invoke<string>("git_base_diff", { path, base, includeWorkingTree }),
  getInstanceBaseBranch: (instanceId: UUID) =>
    invoke<string | null>("get_instance_base_branch", { instanceId }),
  setInstanceBaseBranch: (instanceId: UUID, baseBranch: string | null) =>
    invoke<void>("set_instance_base_branch", { instanceId, baseBranch }),

  discover: () => invoke<DiscoveredProject[]>("discover_claude_projects"),

  allProjectsWithInstances: () => invoke<ProjectWithInstances[]>("all_projects_with_instances"),
  projectWithInstances: (id: UUID) =>
    invoke<ProjectWithInstances>("project_with_instances", { id }),
  instanceStatus: (projectId: UUID, instanceId: UUID) =>
    invoke<InstanceWithStatus>("instance_status", {
      projectId,
      instanceId,
    }),
  instanceMessages: (projectId: UUID, instanceId: UUID, limit?: number) =>
    invoke<TranscriptMessage[]>("instance_messages", {
      projectId,
      instanceId,
      limit: limit ?? null,
    }),

  getSettings: () => invoke<Settings>("get_settings"),
  setTerminal: (choice: TerminalChoice) => invoke<Settings>("set_terminal", { choice }),
  detectTerminals: () => invoke<TerminalInfo[]>("detect_terminals"),
  setupKitty: () => invoke<KittySetupResult>("setup_kitty"),
  openTerminal: (pid: number | null, cwd: string, projectRoot: string | null) =>
    invoke<OpenTerminalOutcome>("open_terminal", { pid, cwd, projectRoot }),
  sendSessionInput: (
    pid: number | null,
    cwd: string,
    projectRoot: string | null,
    input: SessionInput
  ) => invoke<void>("send_session_input", { pid, cwd, projectRoot, input }),
  readSessionPrompt: (pid: number | null, cwd: string, projectRoot: string | null) =>
    invoke<ScreenPrompt | null>("read_session_prompt", { pid, cwd, projectRoot }),
};
