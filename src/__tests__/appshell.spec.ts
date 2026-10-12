import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const h = vi.hoisted(() => {
  const calls: string[] = [];
  const failInit = { value: false };
  const commands = {
    getActiveRoot: vi.fn(async () => {
      calls.push("getActiveRoot");
      return null;
    }),
    detectRoots: vi.fn(async () => {
      calls.push("detectRoots");
      return [];
    }),
    listInstalledApps: vi.fn(async () => {
      calls.push("listInstalledApps");
      return [];
    }),
    listBuckets: vi.fn(async () => {
      calls.push("listBuckets");
      return [];
    }),
    searchPackages: vi.fn(async () => []),
    refreshPackageIndex: vi.fn(async () => 0),
    fetchIcon: vi.fn(async () => null),
  };
  const init = vi.fn(async () => {
    calls.push("logs.init");
    if (failInit.value) throw new Error("listen 失败");
  });
  return { calls, commands, init, failInit };
});

vi.mock("../api/commands", () => h.commands);
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openPath: vi.fn(), openUrl: vi.fn() }));
vi.mock("../stores/logs", () => ({ useLogsStore: () => ({ init: h.init }) }));
vi.mock("../stores/roots", () => ({
  useRootsStore: () => ({
    load: async () => {
      await h.commands.getActiveRoot();
      await h.commands.detectRoots();
    },
  }),
}));
vi.mock("../stores/apps", () => ({
  useAppsStore: () => ({
    load: async () => {
      await h.commands.listInstalledApps();
    },
  }),
}));
vi.mock("../stores/buckets", () => ({
  useBucketsStore: () => ({
    load: async () => {
      await h.commands.listBuckets();
    },
  }),
}));

import AppShell from "../components/layout/AppShell.vue";
import { useUiStore } from "../stores/ui";

const stubs = {
  SideBar: true,
  TopBar: true,
  LogDrawer: true,
  InstalledPage: true,
  StorePage: true,
  BucketsPage: true,
  MaintenancePage: true,
  SettingsPage: true,
  ProfilesPage: true,
  PlaceholderPage: true,
};

describe("AppShell 启动时序", () => {
  beforeEach(() => {
    h.calls.length = 0;
    h.failInit.value = false;
  });

  it("按日志→根目录→应用→软件源的顺序串行加载", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    mount(AppShell, { global: { plugins: [pinia], stubs } });
    await flushPromises();
    await flushPromises();
    expect(h.calls).toEqual([
      "logs.init",
      "getActiveRoot",
      "detectRoots",
      "listInstalledApps",
      "listBuckets",
    ]);
  });

  it("日志初始化失败仍继续加载根目录、应用与软件源", async () => {
    h.failInit.value = true;
    const pinia = createPinia();
    setActivePinia(pinia);
    mount(AppShell, { global: { plugins: [pinia], stubs } });
    await flushPromises();
    await flushPromises();
    expect(h.calls).toEqual([
      "logs.init",
      "getActiveRoot",
      "detectRoots",
      "listInstalledApps",
      "listBuckets",
    ]);
  });

  it("商店页渲染 StorePage 而非占位", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    useUiStore().setPage("store");
    const wrapper = mount(AppShell, { global: { plugins: [pinia], stubs } });
    expect(wrapper.find("store-page-stub").exists()).toBe(true);
  });

  it("维护页渲染 MaintenancePage 而非占位", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    useUiStore().setPage("maintenance");
    const wrapper = mount(AppShell, { global: { plugins: [pinia], stubs } });
    expect(wrapper.find("maintenance-page-stub").exists()).toBe(true);
  });

  it("配置集页渲染 ProfilesPage 而非占位", () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    useUiStore().setPage("profiles");
    const wrapper = mount(AppShell, { global: { plugins: [pinia], stubs } });
    expect(wrapper.find("profiles-page-stub").exists()).toBe(true);
  });
});
