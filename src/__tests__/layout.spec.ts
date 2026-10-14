import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const messageWarning = vi.hoisted(() => vi.fn());
vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: vi.fn(),
      info: vi.fn(),
      success: vi.fn(),
      warning: messageWarning,
    }),
  };
});

const mocks = vi.hoisted(() => ({
  getScoopEnv: vi.fn().mockResolvedValue(null),
  listInstalledApps: vi.fn().mockResolvedValue([]),
  listBuckets: vi.fn().mockResolvedValue([]),
  getScoopConfig: vi.fn().mockResolvedValue(null),
  runScoop: vi.fn(),
  cancelTask: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import LogDrawer from "../components/layout/LogDrawer.vue";
import SideBar from "../components/layout/SideBar.vue";
import { useLogsStore } from "../stores/logs";
import { useUiStore } from "../stores/ui";

const menuStub = {
  props: ["options"],
  emits: ["update:value"],
  template: `<div><button v-for="o in options" :key="o.key" @click="$emit('update:value', o.key)">{{ o.label }}</button></div>`,
};

describe("布局", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("侧边栏包含 8 个导航项且点击切换页面", async () => {
    const wrapper = mount(SideBar, {
      global: { stubs: { Menu: menuStub } },
    });
    const labels = ["已安装", "商店", "软件源管理", "维护", "环境迁移", "配置集", "备份", "设置"];
    const text = wrapper.text();
    for (const label of labels) {
      expect(text).toContain(label);
    }
    const buttons = wrapper.findAll("button");
    expect(buttons).toHaveLength(8);
    await buttons[7].trigger("click");
    expect(useUiStore().activePage).toBe("settings");
  });

  it("取消任务失败时提示警告", async () => {
    const ui = useUiStore();
    const logs = useLogsStore();
    ui.logDrawerOpen = true;
    logs.start("t1", "scoop install git");
    mocks.cancelTask.mockResolvedValueOnce(false);
    const wrapper = mount(LogDrawer);
    const button = wrapper.findAll("button").find((b) => b.text().includes("取消任务"));
    expect(button).toBeTruthy();
    await button!.trigger("click");
    await flushPromises();
    expect(messageWarning).toHaveBeenCalledWith("任务已结束或不存在");
  });
});
