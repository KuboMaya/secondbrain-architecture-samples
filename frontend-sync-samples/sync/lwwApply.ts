/**
 * Portfolio sample — LWW apply path for SyncRecord → Dexie.
 * Highlights: `remote.version > local.version` wins; equal → skip; decrypt then put.
 * Source: core logic adapted from `src/lib/pwaSync.ts`
 * (transport renamed conceptually to Tailscale/Axum peer sync; UI omitted).
 */

import { localDb } from "../dexie/offlineDb";

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

async function localVersion(domain: string, id: string): Promise<number | undefined> {
  switch (domain) {
    case "tasks":
      return (await localDb.tasks.get(id))?.version;
    // Other domains (schedules, dictionary, inventory, …) follow the same pattern.
    default:
      return undefined;
  }
}

/**
 * Apply one accepted record into IndexedDB.
 * Sample shows `tasks` only; production maps every syncable domain the same way.
 */
async function applyRecord(
  record: SyncRecordWire,
  inner: InnerPayload,
): Promise<"update" | "delete"> {
  const p = inner.payload;
  const deletedAt =
    record.is_deleted || p.deleted_at != null
      ? String(p.deleted_at || inner.updated_at)
      : null;

  if (record.domain === "tasks") {
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
    return deletedAt ? "delete" : "update";
  }

  throw new Error(`unsupported domain ${record.domain}`);
}

/**
 * Walk inbound SyncRecords: LWW gate → decrypt → apply.
 * `decrypt` is injected so this sample stays crypto-agnostic
 * (production uses a non-extractable WebCrypto master key).
 */
export async function applyRemoteRecords(
  records: SyncRecordWire[],
  decrypt: (ciphertext: string) => Promise<string>,
  force = false,
): Promise<SyncApplySummary> {
  const summary: SyncApplySummary = { applied: 0, conflicts: 0, skipped: 0 };

  for (const record of records) {
    const localV = await localVersion(record.domain, record.id);
    if (!lwwAccept(localV, record.version, force)) {
      if (localV != null && record.version === localV) {
        summary.skipped += 1;
      } else {
        summary.conflicts += 1;
      }
      continue;
    }

    let inner: InnerPayload;
    if (record.is_deleted || !record.encrypted_data) {
      inner = {
        updated_at: new Date().toISOString(),
        payload: { deleted_at: new Date().toISOString() },
      };
    } else {
      try {
        const plain = await decrypt(record.encrypted_data);
        const value = JSON.parse(plain) as Record<string, unknown>;
        if (value && typeof value === "object" && "payload" in value) {
          inner = {
            updated_at: String(value.updated_at || new Date().toISOString()),
            payload: (value.payload as Record<string, unknown>) || {},
          };
        } else {
          inner = {
            updated_at: new Date().toISOString(),
            payload: value as Record<string, unknown>,
          };
        }
      } catch {
        summary.conflicts += 1;
        continue;
      }
    }

    try {
      await applyRecord(record, inner);
      summary.applied += 1;
    } catch {
      summary.conflicts += 1;
    }
  }

  return summary;
}
