import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  addRoot,
  applyProfileSettings,
  bucketAdd,
  bucketRemove,
  cancelTask,
  createSnapshot,
  deleteProfile,
  deleteSnapshot,
  detectRoots,
  exportProfile,
  fetchIcon,
  getActiveRoot,
  getAppSettings,
  getMirrorInfo,
  getProfile,
  getScoopConfig,
  importProfile,
  installPackage,
  listBuckets,
  listInstalledApps,
  listKnownBuckets,
  listProfiles,
  listSnapshots,
  listUpdates,
  matchRegistryApps,
  profileDiff,
  refreshPackageIndex,
  registerAppInstall,
  renameProfile,
  restoreSnapshotBuckets,
  restoreSnapshotConfig,
  restoreSnapshotPlan,
  runMaintenance,
  runRegistryUninstall,
  runScoop,
  saveProfileFromCurrent,
  scanInstalledRegistry,
  searchPackages,
  setActiveRoot,
  setAria2,
  setBucketSource,
  setHold,
  setProxy,
  setScoopSource,
  setTheme,
  setUrlProxy,
  testUrlProxy,
  uninstallPackage,
  updatePackage,
} from "../api/commands";

describe("commands 包装", () => {
  beforeEach(() => invokeMock.mockReset().mockResolvedValue(null));

  it("命令名与参数符合后端契约", async () => {
    await detectRoots();
    expect(invokeMock).toHaveBeenLastCalledWith("detect_roots", undefined);

    await getActiveRoot();
    expect(invokeMock).toHaveBeenLastCalledWith("get_active_root", undefined);

    await setActiveRoot("C:\\scoop");
    expect(invokeMock).toHaveBeenLastCalledWith("set_active_root", { path: "C:\\scoop" });

    await addRoot("D:\\scoop");
    expect(invokeMock).toHaveBeenLastCalledWith("add_root", { path: "D:\\scoop" });

    await listInstalledApps();
    expect(invokeMock).toHaveBeenLastCalledWith("list_installed_apps", undefined);

    await listBuckets();
    expect(invokeMock).toHaveBeenLastCalledWith("list_buckets", undefined);

    await getScoopConfig();
    expect(invokeMock).toHaveBeenLastCalledWith("get_scoop_config", undefined);

    await runScoop(["install", "git"], "t-1");
    expect(invokeMock).toHaveBeenLastCalledWith("run_scoop", { args: ["install", "git"], taskId: "t-1" });

    await cancelTask("t-1");
    expect(invokeMock).toHaveBeenLastCalledWith("cancel_task", { taskId: "t-1" });
  });

  it("M2 命令名与参数符合后端契约", async () => {
    await searchPackages("git");
    expect(invokeMock).toHaveBeenLastCalledWith("search_packages", { query: "git", limit: undefined });

    await searchPackages("git", 20);
    expect(invokeMock).toHaveBeenLastCalledWith("search_packages", { query: "git", limit: 20 });

    await refreshPackageIndex();
    expect(invokeMock).toHaveBeenLastCalledWith("refresh_package_index", undefined);

    await fetchIcon("https://gitforwindows.org");
    expect(invokeMock).toHaveBeenLastCalledWith("fetch_icon", { source: "https://gitforwindows.org" });
  });

  it("M3 操作命令名与参数符合后端契约", async () => {
    await installPackage("git", "main", null, false, "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("install_package", {
      name: "git",
      bucket: "main",
      version: null,
      global: false,
      taskId: "t-2",
    });

    await uninstallPackage("git", false, true, "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("uninstall_package", {
      name: "git",
      global: false,
      purge: true,
      taskId: "t-2",
    });

    await updatePackage(null, true, false, "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("update_package", {
      name: null,
      all: true,
      global: false,
      taskId: "t-2",
    });

    await setHold("git", true, false, "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("set_hold", {
      name: "git",
      hold: true,
      global: false,
      taskId: "t-2",
    });

    await bucketAdd("java", null, "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("bucket_add", {
      name: "java",
      repo: null,
      taskId: "t-2",
    });

    await bucketRemove("java", "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("bucket_remove", { name: "java", taskId: "t-2" });

    await runMaintenance("checkup", "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("run_maintenance", { op: "checkup", taskId: "t-2" });
  });

  it("M3c 已知软件源命令名符合后端契约", async () => {
    await listKnownBuckets();
    expect(invokeMock).toHaveBeenLastCalledWith("list_known_buckets", undefined);
  });

  it("M4a 更新列表命令名符合后端契约", async () => {
    await listUpdates();
    expect(invokeMock).toHaveBeenLastCalledWith("list_updates", undefined);
  });

  it("M4b 镜像源命令名与参数符合后端契约", async () => {
    await getMirrorInfo();
    expect(invokeMock).toHaveBeenLastCalledWith("get_mirror_info", undefined);

    await setBucketSource("main", "gitee");
    expect(invokeMock).toHaveBeenLastCalledWith("set_bucket_source", { bucket: "main", source: "gitee" });

    await setScoopSource("official");
    expect(invokeMock).toHaveBeenLastCalledWith("set_scoop_source", { source: "official" });

    await setUrlProxy(null);
    expect(invokeMock).toHaveBeenLastCalledWith("set_url_proxy", { value: null });

    await setProxy("127.0.0.1:7890");
    expect(invokeMock).toHaveBeenLastCalledWith("set_proxy", { value: "127.0.0.1:7890" });

    await setAria2(true, 5, 5);
    expect(invokeMock).toHaveBeenLastCalledWith("set_aria2", { enabled: true, split: 5, maxConnection: 5 });

    await testUrlProxy(null);
    expect(invokeMock).toHaveBeenLastCalledWith("test_url_proxy", { proxy: null });
  });

  it("M5a 配置集命令名与参数符合后端契约", async () => {
    await listProfiles();
    expect(invokeMock).toHaveBeenLastCalledWith("list_profiles", undefined);

    await getProfile("工作");
    expect(invokeMock).toHaveBeenLastCalledWith("get_profile", { name: "工作" });

    await saveProfileFromCurrent("工作");
    expect(invokeMock).toHaveBeenLastCalledWith("save_profile_from_current", { name: "工作" });

    await importProfile("C:\\a.json", "工作");
    expect(invokeMock).toHaveBeenLastCalledWith("import_profile", { path: "C:\\a.json", name: "工作" });

    await exportProfile("工作", "C:\\out.json");
    expect(invokeMock).toHaveBeenLastCalledWith("export_profile", { name: "工作", path: "C:\\out.json" });

    await renameProfile("工作", "新工作");
    expect(invokeMock).toHaveBeenLastCalledWith("rename_profile", { name: "工作", newName: "新工作" });

    await deleteProfile("工作");
    expect(invokeMock).toHaveBeenLastCalledWith("delete_profile", { name: "工作" });

    await profileDiff("工作");
    expect(invokeMock).toHaveBeenLastCalledWith("profile_diff", { name: "工作" });

    await applyProfileSettings({ url_proxy: "https://p" });
    expect(invokeMock).toHaveBeenLastCalledWith("apply_profile_settings", {
      settings: { url_proxy: "https://p" },
    });
  });

  it("M5b 快照命令名与参数符合后端契约", async () => {
    await listSnapshots();
    expect(invokeMock).toHaveBeenLastCalledWith("list_snapshots", undefined);

    await createSnapshot();
    expect(invokeMock).toHaveBeenLastCalledWith("create_snapshot", undefined);

    await deleteSnapshot("100");
    expect(invokeMock).toHaveBeenLastCalledWith("delete_snapshot", { id: "100" });

    await restoreSnapshotPlan("100");
    expect(invokeMock).toHaveBeenLastCalledWith("restore_snapshot_plan", { id: "100" });

    await restoreSnapshotConfig("100");
    expect(invokeMock).toHaveBeenLastCalledWith("restore_snapshot_config", { id: "100" });

    await restoreSnapshotBuckets("100");
    expect(invokeMock).toHaveBeenLastCalledWith("restore_snapshot_buckets", { id: "100" });
  });

  it("M6 迁移命令名与参数符合后端契约", async () => {
    await scanInstalledRegistry();
    expect(invokeMock).toHaveBeenLastCalledWith("scan_installed_registry", undefined);

    await matchRegistryApps();
    expect(invokeMock).toHaveBeenLastCalledWith("match_registry_apps", undefined);

    await runRegistryUninstall("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\X", "t-1");
    expect(invokeMock).toHaveBeenLastCalledWith("run_registry_uninstall", {
      entryKey: "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\X",
      taskId: "t-1",
    });
  });

  it("M7 应用设置命令名与参数符合后端契约", async () => {
    await getAppSettings();
    expect(invokeMock).toHaveBeenLastCalledWith("get_app_settings", undefined);

    await setTheme("dark");
    expect(invokeMock).toHaveBeenLastCalledWith("set_theme", { theme: "dark" });
  });

  it("安装钩子命令名与参数符合后端契约", async () => {
    await registerAppInstall("git", true);
    expect(invokeMock).toHaveBeenLastCalledWith("register_app_install", { name: "git", global: true });
  });
});
