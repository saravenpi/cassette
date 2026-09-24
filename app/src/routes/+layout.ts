// Tauri serves the app from disk, so there is no server: SPA mode, no SSR.
// `ssr = false` also means Tauri's IPC is available during `load`.
export const ssr = false;
export const prerender = false;