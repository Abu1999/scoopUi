import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  detectRoots: vi.fn().mockResolvedValue([]),
  getActiveRoot: vi.fn().mockResolvedValue(null),
  setActiveRoot: vi.fn(),
  addRoot: vi.fn(),
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

import SideBar from "../components/layout/SideBar.vue";
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
    const labels = ["已安装", "商店", "桶管理", "维护", "环境迁移", "配置集", "备份", "设置"];
    const text = wrapper.text();
    for (const label of labels) {
      expect(text).toContain(label);
    }
    const buttons = wrapper.findAll("button");
    expect(buttons).toHaveLength(8);
    await buttons[7].trigger("click");
    expect(useUiStore().activePage).toBe("settings");
  });
});
