import type {
  BucketInfo,
  InstalledApp,
  MirrorInfo,
  PackageEntry,
  ProxyTest,
  RootInfo,
  ScoopConfig,
  UpdateInfo,
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
export const searchPackages = (query: string, limit?: number) =>
  invoke<PackageEntry[]>("search_packages", { query, limit });
export const refreshPackageIndex = () => invoke<number>("refresh_package_index");
export const fetchIcon = (source: string) => invoke<string | null>("fetch_icon", { source });
export const installPackage = (
  name: string,
  bucket: string | null,
  version: string | null,
  global: boolean,
  taskId: string,
) => invoke<void>("install_package", { name, bucket, version, global, taskId });
export const uninstallPackage = (name: string, global: boolean, purge: boolean, taskId: string) =>
  invoke<void>("uninstall_package", { name, global, purge, taskId });
export const updatePackage = (name: string | null, all: boolean, global: boolean, taskId: string) =>
  invoke<void>("update_package", { name, all, global, taskId });
export const setHold = (name: string, hold: boolean, global: boolean, taskId: string) =>
  invoke<void>("set_hold", { name, hold, global, taskId });
export const bucketAdd = (name: string, repo: string | null, taskId: string) =>
  invoke<void>("bucket_add", { name, repo, taskId });
export const bucketRemove = (name: string, taskId: string) =>
  invoke<void>("bucket_remove", { name, taskId });
export const runMaintenance = (op: string, taskId: string) =>
  invoke<void>("run_maintenance", { op, taskId });
export const listKnownBuckets = () => invoke<string[]>("list_known_buckets");
export const listUpdates = () => invoke<UpdateInfo[]>("list_updates");
export const getMirrorInfo = () => invoke<MirrorInfo>("get_mirror_info");
export const setBucketSource = (bucket: string, source: string) =>
  invoke<string>("set_bucket_source", { bucket, source });
export const setScoopSource = (source: string) => invoke<string>("set_scoop_source", { source });
export const setUrlProxy = (value: string | null) => invoke<void>("set_url_proxy", { value });
export const setProxy = (value: string | null) => invoke<void>("set_proxy", { value });
export const setAria2 = (enabled: boolean, split: number | null, maxConnection: number | null) =>
  invoke<void>("set_aria2", { enabled, split, maxConnection });
export const testUrlProxy = (proxy: string | null) => invoke<ProxyTest>("test_url_proxy", { proxy });
