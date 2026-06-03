import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type UUID } from "./tauri";

export const qk = {
  projects: ["projects"] as const,
  /** Root key for ALL status queries — invalidating this also clears
   * per-instance status and per-instance messages. Use after mutations that
   * change which projects/instances exist (add, remove, pin, rename). */
  statusRoot: ["status"] as const,
  /** The sidebar list specifically. */
  status: ["status", "all"] as const,
  instance: (projectId: UUID, instanceId: UUID) =>
    ["status", "instance", projectId, instanceId] as const,
  messages: (projectId: UUID, instanceId: UUID) =>
    ["status", "messages", projectId, instanceId] as const,
  prompt: (projectId: UUID, instanceId: UUID) =>
    ["status", "prompt", projectId, instanceId] as const,
  diff: (path: string, scope: string) => ["diff", path, scope] as const,
  baseDiff: (path: string, base: string, includeWorkingTree: boolean) =>
    ["diff", "base", path, base, includeWorkingTree] as const,
  defaultBranch: (path: string) => ["branches", "default", path] as const,
  branches: (path: string) => ["branches", "list", path] as const,
  instanceBase: (instanceId: UUID) => ["instance-base", instanceId] as const,
  discover: ["discover"] as const,
};

export function useProjects() {
  return useQuery({
    queryKey: qk.projects,
    queryFn: () => api.listProjects(),
  });
}

export function useAllStatuses() {
  return useQuery({
    queryKey: qk.status,
    queryFn: () => api.allProjectsWithInstances(),
    refetchInterval: 12_000,
  });
}

export function useInstanceStatus(projectId: UUID | undefined, instanceId: UUID | undefined) {
  return useQuery({
    queryKey:
      projectId && instanceId ? qk.instance(projectId, instanceId) : ["status", "instance", "noop"],
    queryFn: () => api.instanceStatus(projectId!, instanceId!),
    enabled: !!projectId && !!instanceId,
    refetchInterval: 5_000,
  });
}

export function useInstanceMessages(
  projectId: UUID | undefined,
  instanceId: UUID | undefined,
  limit = 40
) {
  return useQuery({
    queryKey:
      projectId && instanceId
        ? ([...qk.messages(projectId, instanceId), limit] as const)
        : (["status", "messages", "noop"] as const),
    queryFn: () => api.instanceMessages(projectId!, instanceId!, limit),
    enabled: !!projectId && !!instanceId,
    refetchInterval: 5_000,
  });
}

/** Poll the session's live terminal screen for a TUI selection prompt
 * (permission request / plan approval). Faster cadence than the transcript
 * since it's interactive; only runs while there's a session to read. */
export function useSessionPrompt(
  projectId: UUID | undefined,
  instanceId: UUID | undefined,
  pid: number | null,
  cwd: string,
  projectRoot: string | null,
  enabled: boolean
) {
  return useQuery({
    queryKey:
      projectId && instanceId ? qk.prompt(projectId, instanceId) : ["status", "prompt", "noop"],
    queryFn: () => api.readSessionPrompt(pid, cwd, projectRoot),
    enabled: !!projectId && !!instanceId && enabled,
    refetchInterval: 2_000,
  });
}

export type DiffScope = "unstaged" | "staged" | "untracked";

export function useGitDiff(path: string | undefined, scope: DiffScope, enabled: boolean) {
  return useQuery({
    queryKey: path ? qk.diff(path, scope) : ["diff", "noop"],
    queryFn: () => api.gitDiff(path!, scope),
    enabled: !!path && enabled,
    refetchInterval: enabled ? 5_000 : false,
  });
}

export function useDiscover(enabled: boolean) {
  return useQuery({
    queryKey: qk.discover,
    queryFn: () => api.discover(),
    enabled,
  });
}

/** Auto-detected default/base branch for a repo (origin/HEAD → main/master/…). */
export function useDefaultBranch(path: string | undefined, enabled: boolean) {
  return useQuery({
    queryKey: path ? qk.defaultBranch(path) : ["branches", "default", "noop"],
    queryFn: () => api.gitDefaultBranch(path!),
    enabled: !!path && enabled,
  });
}

/** Local + remote branches, for the base-branch override picker. */
export function useBranches(path: string | undefined, enabled: boolean) {
  return useQuery({
    queryKey: path ? qk.branches(path) : ["branches", "list", "noop"],
    queryFn: () => api.gitBranches(path!),
    enabled: !!path && enabled,
  });
}

/** Persisted per-instance base-branch override (`null` = use auto-detected). */
export function useInstanceBaseOverride(instanceId: UUID | undefined) {
  return useQuery({
    queryKey: instanceId ? qk.instanceBase(instanceId) : ["instance-base", "noop"],
    queryFn: () => api.getInstanceBaseBranch(instanceId!),
    enabled: !!instanceId,
  });
}

/** Mutation to set/clear the override; invalidates the override + its base diffs. */
export function useSetInstanceBaseOverride(instanceId: UUID | undefined, path: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (baseBranch: string | null) => api.setInstanceBaseBranch(instanceId!, baseBranch),
    onSuccess: () => {
      if (instanceId) qc.invalidateQueries({ queryKey: qk.instanceBase(instanceId) });
      // Prefix match: invalidate every base diff for this path, across both the
      // committed-only and working-tree variants (qk.baseDiff appends base + bool).
      if (path) qc.invalidateQueries({ queryKey: ["diff", "base", path] });
    },
  });
}

export function useBaseDiff(
  path: string | undefined,
  base: string | undefined,
  includeWorkingTree: boolean,
  enabled: boolean
) {
  return useQuery({
    queryKey: path && base ? qk.baseDiff(path, base, includeWorkingTree) : ["diff", "base", "noop"],
    queryFn: () => api.gitBaseDiff(path!, base!, includeWorkingTree),
    enabled: !!path && !!base && enabled,
    // Heavier than the working-tree diffs (merge-base + a full base..HEAD diff)
    // and the base moves rarely, so poll at half the cadence of useGitDiff.
    refetchInterval: enabled ? 10_000 : false,
  });
}
