import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const selectStub = vi.hoisted(() => ({
  props: ["value", "options", "loading", "placeholder"],
  template: `<div class="select-stub" />`,
}));

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    NSelect: selectStub,
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
import type { RootInfo } from "../api/types";

function makeRoot(path: string, isActive: boolean): RootInfo {
  return {
    path,
    valid: true,
    source: "env",
    is_active: isActive,
    app_count: 0,
    bucket_count: 0,
    has_portable_config: false,
    global_dir: null,
  };
}

describe("TopBar 根目录选中值", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("active 带尾分隔符时取列表中的路径而非 placeholder", () => {
    const roots = useRootsStore();
    roots.active = makeRoot("C:\\scoop\\", true);
    roots.roots = [makeRoot("C:\\scoop", true)];
    const wrapper = mount(TopBar);
    const select = wrapper.findComponent(selectStub);
    expect(select.props("value")).toBe("C:\\scoop");
    expect(select.props("value")).not.toBeNull();
  });

  it("无根目录时值为 null", () => {
    const roots = useRootsStore();
    roots.active = null;
    roots.roots = [];
    const wrapper = mount(TopBar);
    expect(wrapper.findComponent(selectStub).props("value")).toBeNull();
  });
});
