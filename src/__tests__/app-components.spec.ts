import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AppDetail from "../components/app/AppDetail.vue";
import AppIcon from "../components/app/AppIcon.vue";
import AppList from "../components/app/AppList.vue";
import InstalledPage from "../pages/InstalledPage.vue";
import SettingsPage from "../pages/SettingsPage.vue";
import { avatarColor } from "../utils/avatar";
import { useAppsStore } from "../stores/apps";
import { useRootsStore } from "../stores/roots";
import type { InstalledApp, RootInfo } from "../api/types";

const messageError = vi.hoisted(() => vi.fn());

const listInstalledApps = vi.hoisted(() => vi.fn());
const getScoopConfig = vi.hoisted(() => vi.fn());
const runScoop = vi.hoisted(() => vi.fn());

vi.mock("../api/commands", () => ({
  listInstalledApps,
  getScoopConfig,
  runScoop,
}));

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: messageError,
      info: vi.fn(),
      success: vi.fn(),
      warning: vi.fn(),
    }),
  };
});

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: vi.fn(() => Promise.reject(new Error("denied"))),
  openUrl: vi.fn(() => Promise.reject(new Error("denied"))),
}));

const git: InstalledApp = {
  name: "git",
  version: "2.55.0.5",
  bucket: "main",
  architecture: "64bit",
  hold: false,
  updated: "2026-03-25T10:16:52Z",
  description: "分布式版本控制系统",
  homepage: "https://gitforwindows.org",
  icon: null,
  depends: ["less"],
  persist: ["etc/gitconfig"],
  bin: ["git"],
  app_dir: "C:\\scoop\\apps\\git",
};

describe("AppIcon", () => {
  it("无图标时显示首字母与稳定背景色", () => {
    const wrapper = mount(AppIcon, { props: { name: "git" } });
    expect(wrapper.text()).toContain("G");
    const probe = document.createElement("div");
    probe.style.background = avatarColor("git");
    const style = wrapper.find(".icon").attributes("style") ?? "";
    expect(style).toContain(probe.style.background);
  });
});

describe("AppList", () => {
  it("渲染应用行并点击 emit select", async () => {
    const wrapper = mount(AppList, { props: { apps: [git], selected: null } });
    expect(wrapper.text()).toContain("git");
    expect(wrapper.text()).toContain("2.55.0.5");
    await wrapper.find(".row").trigger("click");
    expect(wrapper.emitted("select")?.[0]).toEqual(["git"]);
  });
});

describe("AppDetail", () => {
  it("渲染详情字段", () => {
    const wrapper = mount(AppDetail, { props: { app: git } });
    const text = wrapper.text();
    expect(text).toContain("2.55.0.5");
    expect(text).toContain("分布式版本控制系统");
    expect(text).toContain("less");
    expect(text).toContain("etc/gitconfig");
  });

  it("空选中显示空态", () => {
    const wrapper = mount(AppDetail, { props: { app: null } });
    expect(wrapper.text()).toContain("选择一个应用");
  });

  beforeEach(() => {
    messageError.mockClear();
  });

  it("打开安装目录失败时提示错误", async () => {
    const wrapper = mount(AppDetail, { props: { app: git } });
    const button = wrapper.findAll("button").find((b) => b.text().includes("打开安装目录"));
    expect(button).toBeTruthy();
    await button!.trigger("click");
    await flushPromises();
    expect(messageError).toHaveBeenCalledWith("打开失败：无法打开目录");
  });

  it("打开主页失败时提示错误", async () => {
    const wrapper = mount(AppDetail, { props: { app: git } });
    const button = wrapper.findAll("button").find((b) => b.text().includes("打开主页"));
    expect(button).toBeTruthy();
    await button!.trigger("click");
    await flushPromises();
    expect(messageError).toHaveBeenCalledWith("打开失败：无法打开链接");
  });
});

describe("InstalledPage", () => {
  it("加载失败时展示错误文本", async () => {
    listInstalledApps.mockRejectedValueOnce("未找到 scoop 根目录");
    const pinia = createPinia();
    setActivePinia(pinia);
    const apps = useAppsStore();
    await apps.load();
    const wrapper = mount(InstalledPage, { global: { plugins: [pinia] } });
    expect(wrapper.text()).toContain("未找到 scoop 根目录");
  });
});

describe("SettingsPage", () => {
  it("切换根目录后重新加载配置", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const roots = useRootsStore();
    const rootA: RootInfo = {
      path: "C:\\scoop",
      valid: true,
      source: "env",
      is_active: true,
      app_count: 1,
      bucket_count: 1,
      has_portable_config: false,
      global_dir: null,
    };
    roots.active = rootA;
    getScoopConfig.mockResolvedValueOnce({
      file_path: "C:\\scoop\\config.json",
      portable: false,
      values: {},
    });
    const wrapper = mount(SettingsPage, { global: { plugins: [pinia] } });
    await flushPromises();
    expect(getScoopConfig).toHaveBeenCalledTimes(1);
    expect(wrapper.text()).toContain("C:\\scoop\\config.json");
    roots.active = { ...rootA, path: "D:\\scoop", is_active: true };
    getScoopConfig.mockResolvedValueOnce({
      file_path: "D:\\scoop\\config.json",
      portable: true,
      values: {},
    });
    await flushPromises();
    expect(getScoopConfig).toHaveBeenCalledTimes(2);
    expect(wrapper.text()).toContain("D:\\scoop\\config.json");
  });
});
