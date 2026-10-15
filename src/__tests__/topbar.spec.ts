import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: vi.fn(),
      info: vi.fn(),
      success: vi.fn(),
      warning: vi.fn(),
    }),
  };
});

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

import TopBar from "../components/layout/TopBar.vue";
import { useRootsStore } from "../stores/roots";
import type { ScoopEnvInfo } from "../api/types";

function makeEnv(extra: Partial<ScoopEnvInfo> = {}): ScoopEnvInfo {
  return {
    path: "C:\\scoop",
    valid: true,
    app_count: 9,
    bucket_count: 4,
    has_portable_config: false,
    ...extra,
  };
}

describe("TopBar 环境展示", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("只读展示当前环境路径与应用数", () => {
    const roots = useRootsStore();
    roots.env = makeEnv();
    const wrapper = mount(TopBar);
    expect(wrapper.text()).toContain("C:\\scoop");
    expect(wrapper.text()).toContain("9 应用");
    expect(wrapper.findComponent({ name: "Select" }).exists()).toBe(false);
    expect(wrapper.text()).not.toContain("添加根目录");
  });

  it("无环境时显示占位文案", () => {
    const roots = useRootsStore();
    roots.env = null;
    const wrapper = mount(TopBar);
    expect(wrapper.text()).toContain("未找到 scoop 安装目录");
  });

  it("无效环境时展示标签", () => {
    const roots = useRootsStore();
    roots.env = makeEnv({ valid: false });
    const wrapper = mount(TopBar);
    expect(wrapper.text()).toContain("无效");
    expect(wrapper.text()).not.toContain("便携配置");
  });
});
