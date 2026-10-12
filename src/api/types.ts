export interface RootInfo {
  path: string;
  valid: boolean;
  source: string;
  is_active: boolean;
  app_count: number;
  bucket_count: number;
  has_portable_config: boolean;
  global_dir: string | null;
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
  global: boolean;
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
  global: boolean;
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

export interface BucketRef {
  name: string;
  source: string | null;
}

export interface ProfileApp {
  name: string;
  bucket: string | null;
  global: boolean;
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

export interface SnapshotSummary {
  id: string;
  created: number;
  root: string;
  app_count: number;
  bucket_count: number;
  profile_count: number;
}

export interface SnapshotApp {
  name: string;
  bucket: string | null;
  version: string;
  hold: boolean;
  global: boolean;
}

export interface RestorePlan {
  root: string;
  root_matches: boolean;
  config_path: string;
  config_present: boolean;
  reset_buckets: BucketRef[];
  missing_buckets: BucketRef[];
  missing_apps: SnapshotApp[];
}

export interface RestoreBucketsResult {
  reset: string[];
  missing: BucketRef[];
  skipped: string[];
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
