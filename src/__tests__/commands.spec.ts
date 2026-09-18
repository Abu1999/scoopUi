import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  addRoot,
  bucketAdd,
  bucketRemove,
  cancelTask,
  detectRoots,
  fetchIcon,
  getActiveRoot,
  getScoopConfig,
  installPackage,
  listBuckets,
  listInstalledApps,
  refreshPackageIndex,
  runMaintenance,
  runScoop,
  searchPackages,
  setActiveRoot,
  setHold,
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
});
