/**
 * Portfolio sample — LWW SyncRecord apply into Dexie (multi-domain).
 * Cyclomatic weight: version gate, decrypt branches, per-domain put mapping.
 * Source: adapted from `src/lib/pwaSync.ts` (transport-agnostic; no private URLs).
 */

import { localDb, type CachedTask } from "../dexie/offlineDb";

export type SyncRecordWire = {
  id: string;
  domain: string;
  version: number;
  is_deleted: boolean;
  encrypted_data: string;
};

export type SyncApplySummary = {
  applied: number;
  conflicts: number;
  skipped: number;
  results: Array<{
    ok: boolean;
    action?: string;
    id?: string;
    domain?: string;
    error?: string;
    local_version?: number;
    remote_version?: number;
  }>;
};

type InnerPayload = {
  updated_at: string;
  payload: Record<string, unknown>;
};

/** Last-Write-Wins: accept remote only when strictly newer (unless force). */
export function lwwAccept(
  localVersion: number | undefined,
  remoteVersion: number,
  force = false,
): boolean {
  if (force) return true;
  if (localVersion == null) return true;
  return remoteVersion > localVersion;
}

export async function localVersion(
  domain: string,
  id: string,
): Promise<number | undefined> {
  switch (domain) {
    case "tasks":
      return (await localDb.tasks.get(id))?.version;
    case "schedules":
      return (await localDb.schedules.get(id))?.version;
    case "dictionary":
      return (await localDb.knowledge.get(id))?.version;
    case "dictionary_indexes":
      return (await localDb.knowledgeIndexes.get(id))?.version;
    case "inventory":
      return (await localDb.inventoryItems.get(id))?.version;
    case "inventory_categories":
      return (await localDb.inventoryCategories.get(id))?.version;
    case "sleep_records":
      return (await localDb.sleepRecords.get(id))?.version;
    case "morning_routine_config":
      return (await localDb.morningConfig.get(id))?.version;
    case "reward_state":
      return (await localDb.rewardState.get(id))?.version;
    default:
      return undefined;
  }
}

async function applyRecord(
  record: SyncRecordWire,
  inner: InnerPayload,
): Promise<"insert" | "update" | "delete"> {
  const p = inner.payload;
  const deletedAt =
    record.is_deleted || p.deleted_at != null
      ? String(p.deleted_at || inner.updated_at)
      : null;

  switch (record.domain) {
    case "tasks": {
      await localDb.tasks.put({
        id: record.id,
        title: String(p.title ?? ""),
        status: String(p.status ?? "todo"),
        priority: Number(p.priority ?? 0),
        due_date: (p.due_date as string | null) ?? null,
        parent_id: (p.parent_id as string | null) ?? null,
        tags: typeof p.tags === "string" ? p.tags : JSON.stringify(p.tags ?? []),
        reward_multiplier: Number(p.reward_multiplier ?? 1),
        updatedAt: inner.updated_at,
        version: record.version,
        deletedAt,
      });
      break;
    }
    case "schedules": {
      await localDb.schedules.put({
        id: record.id,
        title: String(p.title ?? ""),
        start_time: String(p.start_time ?? ""),
        end_time: String(p.end_time ?? ""),
        is_all_day: Number(p.is_all_day ?? 0),
        recurrence_rule: (p.recurrence_rule as string | null) ?? null,
        location: (p.location as string | null) ?? null,
        notes: (p.notes as string | null) ?? null,
        updatedAt: inner.updated_at,
        version: record.version,
        deletedAt,
      });
      break;
    }
    case "dictionary": {
      await localDb.knowledge.put({
        id: record.id,
        index_id: String(p.index_id ?? ""),
        keyword: String(p.keyword ?? ""),
        content: String(p.content ?? ""),
        source_url: (p.source_url as string | null) ?? null,
        tags: typeof p.tags === "string" ? p.tags : JSON.stringify(p.tags ?? []),
        ai_summary: (p.ai_summary as string | null) ?? null,
        english_content: (p.english_content as string | null) ?? null,
        createdAt: inner.updated_at,
        updatedAt: inner.updated_at,
        version: record.version,
        deletedAt,
      });
      break;
    }
    case "dictionary_indexes": {
      await localDb.knowledgeIndexes.put({
        id: record.id,
        name: String(p.name ?? ""),
        parent_id: (p.parent_id as string | null) ?? null,
        createdAt: inner.updated_at,
        updatedAt: inner.updated_at,
        version: record.version,
        deletedAt,
      });
      break;
    }
    case "inventory": {
      await localDb.inventoryItems.put({
        id: record.id,
        category_id: String(p.category_id ?? ""),
        item_name: String(p.item_name ?? ""),
        status: String(p.status ?? "ok"),
        remaining_amount: Number(p.remaining_amount ?? 0),
        unit: (p.unit as string | null) ?? null,
        notes: (p.notes as string | null) ?? null,
        last_purchased: (p.last_purchased as string | null) ?? null,
        createdAt: inner.updated_at,
        updatedAt: inner.updated_at,
        version: record.version,
        deletedAt,
      });
      break;
    }
    case "inventory_categories": {
      await localDb.inventoryCategories.put({
        id: record.id,
        name: String(p.name ?? ""),
        parent_id: (p.parent_id as string | null) ?? null,
        createdAt: inner.updated_at,
        updatedAt: inner.updated_at,
        version: record.version,
        deletedAt,
      });
      break;
    }
    case "sleep_records": {
      await localDb.sleepRecords.put({
        id: record.id,
        target_date: String(p.target_date ?? ""),
        alarm_time: (p.alarm_time as string | null) ?? null,
        is_alarm_active: Number(p.is_alarm_active ?? 0),
        sleep_start: (p.sleep_start as string | null) ?? null,
        sleep_end: (p.sleep_end as string | null) ?? null,
        createdAt: inner.updated_at,
        updatedAt: inner.updated_at,
        version: record.version,
        deletedAt,
      });
      break;
    }
    case "morning_routine_config": {
      const existing = await localDb.morningConfig.get(record.id);
      await localDb.morningConfig.put({
        id: record.id,
        is_enabled: Number(p.is_enabled ?? 1),
        // Device-local path: never overwrite from peer sync.
        voiceroid_path: existing?.voiceroid_path ?? null,
        weather_location: (p.weather_location as string | null) ?? null,
        read_aloud_calendar: Number(p.read_aloud_calendar ?? 1),
        createdAt: existing?.createdAt ?? inner.updated_at,
        updatedAt: inner.updated_at,
        version: record.version,
        deletedAt,
      });
      break;
    }
    case "reward_state": {
      await localDb.rewardState.put({
        id: record.id,
        hidden_points: Number(p.hidden_points ?? 0),
        monthly_budget: Number(p.monthly_budget ?? 0),
        createdAt: inner.updated_at,
        updatedAt: inner.updated_at,
        version: record.version,
        deletedAt,
      });
      break;
    }
    default:
      throw new Error(`unsupported domain ${record.domain}`);
  }
  return deletedAt ? "delete" : "update";
}

async function decryptRecord(
  record: SyncRecordWire,
  decrypt: (ciphertext: string) => Promise<string>,
): Promise<InnerPayload | null> {
  if (record.is_deleted || !record.encrypted_data) {
    return {
      updated_at: new Date().toISOString(),
      payload: { deleted_at: new Date().toISOString() },
    };
  }
  try {
    const plain = await decrypt(record.encrypted_data);
    const value = JSON.parse(plain) as Record<string, unknown>;
    if (value && typeof value === "object" && "payload" in value) {
      return {
        updated_at: String(value.updated_at || new Date().toISOString()),
        payload: (value.payload as Record<string, unknown>) || {},
      };
    }
    return {
      updated_at: new Date().toISOString(),
      payload: value as Record<string, unknown>,
    };
  } catch {
    return null;
  }
}

/** Apply inbound SyncRecords with LWW gating + per-domain Dexie writes. */
export async function applyRemoteRecords(
  records: SyncRecordWire[],
  decrypt: (ciphertext: string) => Promise<string>,
  force = false,
): Promise<SyncApplySummary> {
  const summary: SyncApplySummary = {
    applied: 0,
    conflicts: 0,
    skipped: 0,
    results: [],
  };

  for (const record of records) {
    const localV = await localVersion(record.domain, record.id);
    if (!lwwAccept(localV, record.version, force)) {
      if (localV != null && record.version === localV) {
        summary.skipped += 1;
        summary.results.push({
          ok: true,
          action: "noop",
          id: record.id,
          domain: record.domain,
        });
      } else {
        summary.conflicts += 1;
        summary.results.push({
          ok: false,
          error: "conflict",
          id: record.id,
          domain: record.domain,
          local_version: localV,
          remote_version: record.version,
        });
      }
      continue;
    }

    const inner = await decryptRecord(record, decrypt);
    if (!inner) {
      summary.conflicts += 1;
      summary.results.push({
        ok: false,
        error: "decrypt_failed",
        id: record.id,
        domain: record.domain,
      });
      continue;
    }

    try {
      const action = await applyRecord(record, inner);
      summary.applied += 1;
      summary.results.push({
        ok: true,
        action,
        id: record.id,
        domain: record.domain,
      });
    } catch (e) {
      summary.conflicts += 1;
      summary.results.push({
        ok: false,
        error: e instanceof Error ? e.message : "apply_failed",
        id: record.id,
        domain: record.domain,
      });
    }
  }

  return summary;
}

/** Optimistic local task write used before Outbox flush (UI path). */
export async function upsertLocalTaskCache(task: CachedTask): Promise<void> {
  const existing = await localDb.tasks.get(task.id);
  if (existing && existing.version > task.version) {
    return; // local newer — LWW keep
  }
  await localDb.tasks.put(task);
}
