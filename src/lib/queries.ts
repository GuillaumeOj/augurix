import { useQuery } from "@tanstack/react-query";
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
  diff: (path: string, scope: string) => ["diff", path, scope] as const,
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
