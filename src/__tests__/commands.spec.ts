import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  addRoot,
  cancelTask,
  detectRoots,
  getActiveRoot,
  getScoopConfig,
  listBuckets,
  listInstalledApps,
  runScoop,
  setActiveRoot,
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
});
