export interface ScoopEnvInfo {
  path: string;
  valid: boolean;
  app_count: number;
  bucket_count: number;
  has_portable_config: boolean;
}

export interface InstalledApp {
  name: string;
  version: string;
  bucket: string | null;
  architecture: string | null;
  hold: boolean;
  updated: string | null;
  description: string | null;
  homepage: string | null;
  icon: string | null;
  depends: string[];
  persist: string[];
  bin: string[];
  app_dir: string;
}

export interface BucketInfo {
  name: string;
  source: string | null;
  is_git: boolean;
  manifest_count: number;
}

export interface ScoopConfig {
  file_path: string;
  portable: boolean;
  values: unknown;
}

export interface LockHolderInfo {
  pid: number;
  name: string;
  is_explorer: boolean;
}

export interface EnvLockInfo {
  remaining_files: string[];
  holders: LockHolderInfo[];
}

export interface AppProcessInfo {
  pid: number;
  name: string;
}

export interface ScoopLogEvent {
  taskId: string;
  stream: "stdout" | "stderr";
  line: string;
}

export interface ScoopTaskEndEvent {
  taskId: string;
  code: number | null;
  success: boolean;
}

export interface UpdateInfo {
  name: string;
  current: string;
  latest: string;
  bucket: string;
  hold: boolean;
}

export interface PackageEntry {
  name: string;
  bucket: string;
  version: string;
  description: string | null;
  homepage: string | null;
  license: string | null;
  icon: string | null;
  deprecated: boolean;
}

export interface MirrorPresets {
  official: string;
  gitee: string;
  proxy: string;
}

export interface BucketMirrorInfo {
  name: string;
  remote: string | null;
  presets: MirrorPresets | null;
}

export interface MirrorInfo {
  scoop_repo: string | null;
  scoop_remote: string | null;
  presets: MirrorPresets | null;
  url_proxy: string | null;
  default_url_proxy: string;
  proxy: string | null;
  buckets: BucketMirrorInfo[];
  aria2_installed: boolean;
  aria2_enabled: boolean;
  aria2_split: number | null;
  aria2_max_connection: number | null;
}

export interface ProxyTest {
  exit_info: string | null;
  in_china: boolean | null;
  proxy_reachable: boolean;
  proxy_info: string | null;
}

export interface BundleInfo {
  built: string;
  scoop_commit: string;
  main_commit: string;
}

export interface PrereqReport {
  default_dir: string;
  dir: string;
  dir_ok: boolean;
  dir_reason: string | null;
  powershell_ok: boolean;
  tar_available: boolean;
  git_available: boolean;
  elevated: boolean;
  existing_root: string | null;
}

export interface FinalizeResult {
  manifest_count: number;
  path_updated: boolean;
  warnings: string[];
}

export interface BucketRef {
  name: string;
  source: string | null;
}

export interface ProfileApp {
  name: string;
  bucket: string | null;
}

export interface Profile {
  version: number;
  name: string;
  created: number;
  buckets: BucketRef[];
  apps: ProfileApp[];
  settings: Record<string, unknown>;
}

export interface ProfileSummary {
  name: string;
  created: number;
  bucket_count: number;
  app_count: number;
  settings_count: number;
}

export interface SettingDiff {
  key: string;
  current: unknown | null;
  target: unknown;
}

export interface ProfileDiff {
  missing_buckets: BucketRef[];
  extra_buckets: string[];
  bucket_source_conflicts: string[];
  missing_apps: ProfileApp[];
  extra_apps: ProfileApp[];
  settings: SettingDiff[];
}

export interface RegistryApp {
  key_path: string;
  name: string;
  version: string | null;
  publisher: string | null;
  install_location: string | null;
  uninstall_string: string | null;
  exe_names: string[];
  managed: boolean;
}

export interface MigrationMatch {
  name: string;
  bucket: string;
  confidence: string;
  version: string;
  description: string | null;
}

export interface MigrationEntry {
  app: RegistryApp;
  matches: MigrationMatch[];
}

export interface AppSettings {
  theme: string;
}
