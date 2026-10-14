import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  matchRegistryApps: vi.fn(),
  runRegistryUninstall: vi.fn(),
  installPackage: vi.fn(),
  listInstalledApps: vi.fn(),
  listUpdates: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { useLogsStore } from "../stores/logs";
import { useMigrateStore } from "../stores/migrate";
import type { MigrationEntry } from "../api/types";

function entry(name: string, managed: boolean): MigrationEntry {
  return {
    app: {
      key_path: `HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\${name}`,
      name,
      version: "1.0",
      publisher: null,
      install_location: null,
      uninstall_string: null,
      exe_names: [],
      managed,
    },
    matches: [
      { name: name.toLowerCase(), bucket: "main", confidence: "high", version: "2.0", description: null },
    ],
  };
}

describe("migrate store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([]);
    mocks.listUpdates.mockResolvedValue([]);
  });

  it("scan 填充条目并过滤已管理", async () => {
    mocks.matchRegistryApps.mockResolvedValue([entry("App", false), entry("ScoopManaged", true)]);
    const store = useMigrateStore();
    await store.scan();
    expect(store.scanned).toBe(true);
    expect(store.entries).toHaveLength(2);
    expect(store.visible).toHaveLength(1);
    expect(store.managedCount).toBe(1);
    store.showManaged = true;
    expect(store.visible).toHaveLength(2);
  });

  it("scan 失败时 error 可读", async () => {
    mocks.matchRegistryApps.mockRejectedValue("未找到可用的 scoop 根目录");
    const store = useMigrateStore();
    await store.scan();
    expect(store.entries).toEqual([]);
    expect(store.error).toContain("未找到");
  });

  it("install 勾选时先卸载再安装", async () => {
    mocks.runRegistryUninstall.mockResolvedValue(undefined);
    mocks.installPackage.mockResolvedValue(undefined);
    const store = useMigrateStore();
    const e = entry("App", false);
    const running = store.install(e, e.matches[0], true);
    await flushPromises();
    expect(mocks.runRegistryUninstall).toHaveBeenCalledWith(e.app.key_path, expect.any(String));
    const uninstallTask = mocks.runRegistryUninstall.mock.calls[0][1];
    useLogsStore().end({ taskId: uninstallTask, code: 0, success: true });
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalledWith("app", "main", null, expect.any(String));
    const installTask = mocks.installPackage.mock.calls[0][3];
    useLogsStore().end({ taskId: installTask, code: 0, success: true });
    await flushPromises();
    const result = await running;
    expect(result.ok).toBe(true);
    expect(result.warning).toBeNull();
    expect(mocks.listInstalledApps).toHaveBeenCalled();
  });

  it("卸载失败仅警告并继续安装", async () => {
    mocks.runRegistryUninstall.mockResolvedValue(undefined);
    mocks.installPackage.mockResolvedValue(undefined);
    const store = useMigrateStore();
    const e = entry("App", false);
    const running = store.install(e, e.matches[0], true);
    await flushPromises();
    const uninstallTask = mocks.runRegistryUninstall.mock.calls[0][1];
    useLogsStore().end({ taskId: uninstallTask, code: 1, success: false });
    await flushPromises();
    const installTask = mocks.installPackage.mock.calls[0][3];
    useLogsStore().end({ taskId: installTask, code: 0, success: true });
    await flushPromises();
    const result = await running;
    expect(result.ok).toBe(true);
    expect(result.warning).toContain("旧版卸载失败");
  });
});
