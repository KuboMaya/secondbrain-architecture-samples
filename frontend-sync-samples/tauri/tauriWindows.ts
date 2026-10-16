/**
 * Portfolio sample — secondary Tauri windows (Resident / Taskbar shells).
 * Source: `src/lib/tauriWindows.ts`.
 */

import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, type LayoutMode } from "../layout/layoutMode";

export type OpenableLayout = Extract<LayoutMode, "resident" | "taskbar">;

/** Open Resident / Taskbar secondary window (Tauri only). */
export async function openLayoutWindow(mode: OpenableLayout): Promise<boolean> {
  if (!isTauriRuntime()) return false;
  try {
    await invoke("open_layout_window", { mode });
    return true;
  } catch (err) {
    console.warn("open_layout_window failed", err);
    return false;
  }
}

export async function closeLayoutWindow(mode: OpenableLayout): Promise<boolean> {
  if (!isTauriRuntime()) return false;
  try {
    await invoke("close_layout_window", { mode });
    return true;
  } catch (err) {
    console.warn("close_layout_window failed", err);
    return false;
  }
}
