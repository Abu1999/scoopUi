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

const stubs = {
  SideBar: true,
  TopBar: true,
  LogDrawer: true,
  InstalledPage: true,
  BucketsPage: true,
  SettingsPage: true,
  PlaceholderPage: true,
};

describe("AppShell 启动时序", () => {
  beforeEach(() => {
    h.calls.length = 0;
    h.failInit.value = false;
  });

  it("按日志→根目录→应用→桶的顺序串行加载", async () => {
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

  it("日志初始化失败仍继续加载根目录、应用与桶", async () => {
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
});
