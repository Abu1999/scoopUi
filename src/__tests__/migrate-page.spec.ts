import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const messageError = vi.hoisted(() => vi.fn());
const messageSuccess = vi.hoisted(() => vi.fn());
const messageWarning = vi.hoisted(() => vi.fn());
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
vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: messageError,
      success: messageSuccess,
      warning: messageWarning,
      info: vi.fn(),
    }),
  };
});

import MigratePage from "../pages/MigratePage.vue";
import { useLogsStore } from "../stores/logs";
import { useMigrateStore } from "../stores/migrate";
import { useUiStore } from "../stores/ui";
import type { MigrationEntry } from "../api/types";

function entry(name: string, managed: boolean, confidence = "high"): MigrationEntry {
  return {
    app: {
      key_path: `HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\${name}`,
      name,
      version: "1.0",
      publisher: "Pub",
      install_location: null,
      uninstall_string: null,
      exe_names: [],
      managed,
    },
    matches: [
      { name: name.toLowerCase(), bucket: "main", confidence, version: "2.0", description: null },
    ],
  };
}

describe("MigratePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([]);
    mocks.listUpdates.mockResolvedValue([]);
  });

  it("未扫描显示空态，扫描后渲染分组与匹配", async () => {
    mocks.matchRegistryApps.mockResolvedValue([entry("App", false)]);
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mount(MigratePage, { global: { plugins: [pinia] } });
    expect(wrapper.text()).toContain("扫描本机已安装软件");
    const btn = wrapper.findAll("button").find((b) => b.text().includes("扫描"));
    await btn!.trigger("click");
    await flushPromises();
    expect(mocks.matchRegistryApps).toHaveBeenCalled();
    expect(wrapper.text()).toContain("App");
    expect(wrapper.text()).toContain("高置信匹配");
    expect(wrapper.text()).toContain("main/app");
  });

  it("安装调用 installPackage 并提示成功", async () => {
    mocks.matchRegistryApps.mockResolvedValue([entry("App", false)]);
    mocks.installPackage.mockResolvedValue(undefined);
    const pinia = createPinia();
    setActivePinia(pinia);
    await useMigrateStore().scan();
    const wrapper = mount(MigratePage, { global: { plugins: [pinia] } });
    await flushPromises();
    await wrapper.findAll('[role="checkbox"]')[1].trigger("click");
    const install = wrapper.findAll("button").find((b) => b.text() === "安装");
    await install!.trigger("click");
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][3];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(mocks.runRegistryUninstall).not.toHaveBeenCalled();
    expect(mocks.installPackage).toHaveBeenCalledWith("app", "main", null, expect.any(String));
    expect(messageSuccess).toHaveBeenCalled();
  });

  it("默认勾选先卸载旧版并在安装前调用卸载命令", async () => {
    mocks.matchRegistryApps.mockResolvedValue([entry("App", false)]);
    mocks.runRegistryUninstall.mockResolvedValue(undefined);
    mocks.installPackage.mockResolvedValue(undefined);
    const pinia = createPinia();
    setActivePinia(pinia);
    await useMigrateStore().scan();
    const wrapper = mount(MigratePage, { global: { plugins: [pinia] } });
    await flushPromises();
    const checkboxes = wrapper.findAll('[role="checkbox"]');
    expect(checkboxes[1].classes()).toContain("n-checkbox--checked");
    const install = wrapper.findAll("button").find((b) => b.text() === "安装");
    await install!.trigger("click");
    await flushPromises();
    expect(mocks.runRegistryUninstall).toHaveBeenCalled();
    const uninstallTask = mocks.runRegistryUninstall.mock.calls[0][1];
    useLogsStore().end({ taskId: uninstallTask, code: 0, success: true });
    await flushPromises();
    const installTask = mocks.installPackage.mock.calls[0][3];
    useLogsStore().end({ taskId: installTask, code: 0, success: true });
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalled();
  });

  it("查看跳转商店并携带搜索词", async () => {
    mocks.matchRegistryApps.mockResolvedValue([entry("App", false)]);
    const pinia = createPinia();
    setActivePinia(pinia);
    await useMigrateStore().scan();
    const wrapper = mount(MigratePage, { global: { plugins: [pinia] } });
    await flushPromises();
    const view = wrapper.findAll("button").find((b) => b.text() === "查看");
    await view!.trigger("click");
    expect(useUiStore().storeSearchQuery).toBe("app");
    expect(useUiStore().activePage).toBe("store");
  });

  it("已管理条目的安装按钮禁用", async () => {
    mocks.matchRegistryApps.mockResolvedValue([entry("Managed", true)]);
    const pinia = createPinia();
    setActivePinia(pinia);
    await useMigrateStore().scan();
    useMigrateStore().showManaged = true;
    const wrapper = mount(MigratePage, { global: { plugins: [pinia] } });
    await flushPromises();
    const install = wrapper.findAll("button").find((b) => b.text() === "安装");
    expect(install!.attributes("disabled")).toBeDefined();
  });
});
