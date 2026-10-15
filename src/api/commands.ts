import type {
  AppProcessInfo,
  AppSettings,
  BucketInfo,
  BundleInfo,
  EnvLockInfo,
  FinalizeResult,
  InstalledApp,
  MigrationEntry,
  MirrorInfo,
  PackageEntry,
  PrereqReport,
  Profile,
  ProfileDiff,
  ProfileSummary,
  ProxyTest,
  RegistryApp,
  ScoopConfig,
  ScoopEnvInfo,
  UpdateInfo,
} from "./types";
import { invoke } from "./tauri";

export const getScoopEnv = () => invoke<ScoopEnvInfo | null>("get_scoop_env");
export const adoptScoopDir = (path: string) => invoke<ScoopEnvInfo>("adopt_scoop_dir", { path });
export const moveScoopDir = (newDir: string) => invoke<ScoopEnvInfo>("move_scoop_dir", { newDir });
export const removeScoopEnv = () => invoke<string>("remove_scoop_env");
export const envLockInfo = () => invoke<EnvLockInfo>("env_lock_info");
export const forceRemoveScoopEnv = () => invoke<string[]>("force_remove_scoop_env");
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
  taskId: string,
) => invoke<void>("install_package", { name, bucket, version, taskId });
export const uninstallPackage = (name: string, purge: boolean, taskId: string) =>
  invoke<void>("uninstall_package", { name, purge, taskId });
export const forceUninstallPackage = (name: string, purge: boolean, taskId: string) =>
  invoke<void>("force_uninstall_package", { name, purge, taskId });
export const listAppProcesses = (name: string) =>
  invoke<AppProcessInfo[]>("list_app_processes", { name });
export const updatePackage = (name: string | null, all: boolean, taskId: string) =>
  invoke<void>("update_package", { name, all, taskId });
export const setHold = (name: string, hold: boolean, taskId: string) =>
  invoke<void>("set_hold", { name, hold, taskId });
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
export const registerAppInstall = (name: string) => invoke<string[]>("register_app_install", { name });
export const getScoopBundleInfo = () => invoke<BundleInfo>("get_scoop_bundle_info");
export const checkScoopPrereqs = (dir: string) => invoke<PrereqReport>("check_scoop_prereqs", { dir });
export const extractScoopBundle = (dir: string, taskId: string) =>
  invoke<void>("extract_scoop_bundle", { dir, taskId });
export const finalizeScoopBootstrap = (dir: string, addToPath: boolean) =>
  invoke<FinalizeResult>("finalize_scoop_bootstrap", { dir, addToPath });
export const installScoopOnline = (dir: string, proxy: string | null, taskId: string) =>
  invoke<void>("install_scoop_online", { dir, proxy, taskId });

export const listProfiles = () => invoke<ProfileSummary[]>("list_profiles");
export const getProfile = (name: string) => invoke<Profile>("get_profile", { name });
export const saveProfileFromCurrent = (name: string) => invoke<Profile>("save_profile_from_current", { name });
export const importProfile = (path: string, name: string) => invoke<Profile>("import_profile", { path, name });
export const exportProfile = (name: string, path: string) => invoke<void>("export_profile", { name, path });
export const renameProfile = (name: string, newName: string) => invoke<void>("rename_profile", { name, newName });
export const deleteProfile = (name: string) => invoke<void>("delete_profile", { name });
export const profileDiff = (name: string) => invoke<ProfileDiff>("profile_diff", { name });
export const applyProfileSettings = (settings: Record<string, unknown>) =>
  invoke<void>("apply_profile_settings", { settings });

export const scanInstalledRegistry = () => invoke<RegistryApp[]>("scan_installed_registry");
export const matchRegistryApps = () => invoke<MigrationEntry[]>("match_registry_apps");
export const runRegistryUninstall = (entryKey: string, taskId: string) =>
  invoke<void>("run_registry_uninstall", { entryKey, taskId });

export const getAppSettings = () => invoke<AppSettings>("get_app_settings");
export const setTheme = (theme: string) => invoke<void>("set_theme", { theme });
