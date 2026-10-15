/**
 * Portfolio sample — LayoutMode shell selection (pages stay layout-agnostic).
 * Source: src/lib/device.ts
 */

export type AppEnvironment = "tauri" | "web_pc" | "pwa_mobile";

/**
 * Presentation shell for the React tree.
 * Pages stay layout-agnostic; App selects one of these wrappers.
 *
 * - desktop  — standard PC / Tauri main window
 * - mobile   — phone PWA (CRUD + remote controls; not primary alarm speaker)
 * - resident — always-on desktop widget (clock / next alarm / weather / points)
 * - taskbar  — tray / micro UI (compact status strip)
 */
export type LayoutMode = "desktop" | "mobile" | "resident" | "taskbar";

/** Layout breakpoint aligned with architecture (mobile shell below 768px). */
export const MOBILE_BREAKPOINT_PX = 768;

const LAYOUT_MODES: readonly LayoutMode[] = [
  "desktop",
  "mobile",
  "resident",
  "taskbar",
];

type TauriWindow = Window & {
  __TAURI__?: unknown;
  __TAURI_INTERNALS__?: unknown;
};

/** True when running inside a Tauri webview. */
export function isTauriRuntime(): boolean {
  if (typeof window === "undefined") return false;
  const w = window as TauriWindow;
  return w.__TAURI__ != null || w.__TAURI_INTERNALS__ != null;
}

/**
 * Detect current runtime shell:
 * - `tauri` — desktop Tauri app (`window.__TAURI__`)
 * - `web_pc` — browser / sub-PC web, viewport ≥ 768px
 * - `pwa_mobile` — browser / installed PWA, viewport < 768px
 */
export function detectEnvironment(): AppEnvironment {
  if (typeof window === "undefined") return "web_pc";
  if (isTauriRuntime()) return "tauri";
  if (window.innerWidth < MOBILE_BREAKPOINT_PX) return "pwa_mobile";
  return "web_pc";
}

function parseLayoutToken(raw: string | null | undefined): LayoutMode | null {
  if (!raw) return null;
  const token = raw.trim().toLowerCase();
  return (LAYOUT_MODES as readonly string[]).includes(token)
    ? (token as LayoutMode)
    : null;
}

/**
 * Explicit override from URL:
 * - query: `?layout=resident`
 * - hash: `#layout=taskbar` or `#resident`
 */
export function parseLayoutOverride(
  search: string = typeof window !== "undefined" ? window.location.search : "",
  hash: string = typeof window !== "undefined" ? window.location.hash : "",
): LayoutMode | null {
  try {
    const params = new URLSearchParams(
      search.startsWith("?") ? search.slice(1) : search,
    );
    const fromQuery = parseLayoutToken(params.get("layout"));
    if (fromQuery) return fromQuery;
  } catch {
    // ignore malformed search
  }

  const rawHash = hash.startsWith("#") ? hash.slice(1) : hash;
  if (!rawHash) return null;
  if (rawHash.startsWith("layout=")) {
    return parseLayoutToken(rawHash.slice("layout=".length));
  }
  // Allow bare `#resident` style for compact Tauri window URLs.
  return parseLayoutToken(rawHash.split("&")[0]?.split("=")[0]);
}

/**
 * Resolve which layout shell to mount.
 * Priority: URL override → environment default (mobile PWA vs desktop/Tauri).
 */
export function resolveLayoutMode(options?: {
  search?: string;
  hash?: string;
  environment?: AppEnvironment;
}): LayoutMode {
  const override = parseLayoutOverride(options?.search, options?.hash);
  if (override) return override;

  const env = options?.environment ?? detectEnvironment();
  if (env === "pwa_mobile") return "mobile";
  // Tauri main window and wide web_pc share desktop until multi-window sets ?layout=
  return "desktop";
}

/** PC shells (Tauri + Web_PC) use the desktop layout when no URL override. */
export function usesDesktopLayout(env: AppEnvironment = detectEnvironment()): boolean {
  return resolveLayoutMode({ environment: env }) === "desktop";
}

/** Mobile / PWA uses the mobile layout when no URL override. */
export function usesMobileLayout(env: AppEnvironment = detectEnvironment()): boolean {
  return resolveLayoutMode({ environment: env }) === "mobile";
}

/** @deprecated use `detectEnvironment` / `AppEnvironment` */
export type DeviceKind = "desktop" | "mobile";

/** @deprecated use `detectEnvironment` */
export function detectDevice(): DeviceKind {
  return usesMobileLayout() ? "mobile" : "desktop";
}
