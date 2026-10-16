/**
 * Portfolio sample — offline-first API edge with Dexie fallback + Outbox.
 * Source: adapted from `src/lib/api.ts` (task paths shown; others same pattern).
 */

import {
  enqueueOutbox,
  localDb,
  type CachedTask,
} from "../dexie/offlineDb";
import { authHeaders, getApiBase } from "../sync/syncPolicy";

function apiUrl(path: string): string {
  return `${getApiBase()}${path}`;
}

/** Prefer live Axum; on failure return Dexie/cache fallback (never freeze UI). */
export async function apiGet<T>(path: string, fallback: T): Promise<T> {
  try {
    const res = await fetch(apiUrl(path), {
      headers: authHeaders(),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return (await res.json()) as T;
  } catch {
    return fallback;
  }
}

/**
 * Returns parsed JSON on success; `null` when offline (mutation queued in Outbox).
 */
export async function apiMutateJson<T>(
  method: string,
  path: string,
  body?: unknown,
): Promise<T | null> {
  try {
    const res = await fetch(apiUrl(path), {
      method,
      headers: authHeaders({ "Content-Type": "application/json" }),
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    if (res.status === 204) return null;
    return (await res.json()) as T;
  } catch {
    await enqueueOutbox({ method, path, body });
    return null;
  }
}

type TaskDto = {
  id: string;
  title: string;
  status: string;
  priority: number;
  due_date?: string | null;
  parent_id?: string | null;
  tags?: string;
  reward_multiplier?: number;
  updated_at: string;
  version: number;
  deleted_at?: string | null;
};

function toCachedTask(task: TaskDto): CachedTask {
  return {
    id: task.id,
    title: task.title,
    status: task.status,
    priority: task.priority,
    due_date: task.due_date,
    parent_id: task.parent_id,
    tags: task.tags,
    reward_multiplier: task.reward_multiplier,
    updatedAt: task.updated_at,
    version: task.version,
    deletedAt: task.deleted_at,
  };
}

/** List tasks: network first, else Dexie cache. */
export async function listTasks(): Promise<CachedTask[]> {
  const cached = await localDb.tasks
    .filter((t) => !t.deletedAt)
    .toArray();
  const remote = await apiGet<TaskDto[]>("/tasks", []);
  if (remote.length === 0 && cached.length > 0) {
    return cached;
  }
  for (const row of remote) {
    await localDb.tasks.put(toCachedTask(row));
  }
  return remote.map(toCachedTask);
}

/** Create task; on offline, queue Outbox and write optimistic Dexie row. */
export async function createTask(input: {
  title: string;
  priority?: number;
}): Promise<CachedTask | null> {
  const body = {
    title: input.title,
    priority: input.priority ?? 0,
  };
  const created = await apiMutateJson<TaskDto>("POST", "/tasks", body);
  if (created) {
    const cached = toCachedTask(created);
    await localDb.tasks.put(cached);
    return cached;
  }
  // Offline: optimistic local id + Outbox already enqueued by apiMutateJson.
  const optimistic: CachedTask = {
    id: crypto.randomUUID(),
    title: input.title,
    status: "todo",
    priority: input.priority ?? 0,
    updatedAt: new Date().toISOString(),
    version: 1,
  };
  await localDb.tasks.put(optimistic);
  return optimistic;
}

/** Complete task — mirrors 0ms backend publish path on the client cache. */
export async function completeTask(id: string): Promise<CachedTask | null> {
  const updated = await apiMutateJson<TaskDto>("POST", `/tasks/${id}/complete`);
  if (updated) {
    const cached = toCachedTask(updated);
    await localDb.tasks.put(cached);
    return cached;
  }
  const existing = await localDb.tasks.get(id);
  if (!existing) return null;
  const next = {
    ...existing,
    status: "done",
    version: existing.version + 1,
    updatedAt: new Date().toISOString(),
  };
  await localDb.tasks.put(next);
  return next;
}
