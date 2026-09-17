import type {
  BucketInfo,
  InstalledApp,
  RootInfo,
  ScoopConfig,
} from "./types";
import { invoke } from "./tauri";

export const detectRoots = () => invoke<RootInfo[]>("detect_roots");
export const getActiveRoot = () => invoke<RootInfo | null>("get_active_root");
export const setActiveRoot = (path: string) => invoke<RootInfo>("set_active_root", { path });
export const addRoot = (path: string) => invoke<RootInfo>("add_root", { path });
export const listInstalledApps = () => invoke<InstalledApp[]>("list_installed_apps");
export const listBuckets = () => invoke<BucketInfo[]>("list_buckets");
export const getScoopConfig = () => invoke<ScoopConfig>("get_scoop_config");
export const runScoop = (args: string[], taskId: string) =>
  invoke<void>("run_scoop", { args, taskId });
export const cancelTask = (taskId: string) => invoke<boolean>("cancel_task", { taskId });
