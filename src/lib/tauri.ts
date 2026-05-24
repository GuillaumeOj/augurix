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

export interface ToolUseEntry {
  name: string;
  detail: string | null;
}

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

export const api = {
  listProjects: () => invoke<Project[]>("list_projects"),
  addProject: (path: string) => invoke<Project>("add_project", { path }),
  removeProject: (id: UUID) => invoke<void>("remove_project", { id }),
  renameProject: (id: UUID, name: string) => invoke<Project | null>("rename_project", { id, name }),
  setPinned: (id: UUID, pinned: boolean) => invoke<void>("set_pinned", { id, pinned }),

  gitDiff: (path: string, scope: "unstaged" | "staged" | "untracked") =>
    invoke<string>("git_diff", { path, scope }),

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
};
