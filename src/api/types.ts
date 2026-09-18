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
