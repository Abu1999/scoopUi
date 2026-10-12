import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AppDetail from "../components/app/AppDetail.vue";
import AppIcon from "../components/app/AppIcon.vue";
import AppList from "../components/app/AppList.vue";
import InstalledPage from "../pages/InstalledPage.vue";
import SettingsPage from "../pages/SettingsPage.vue";
import { useAppsStore } from "../stores/apps";
import { useRootsStore } from "../stores/roots";
import type { InstalledApp, MirrorInfo, RootInfo } from "../api/types";

const messageError = vi.hoisted(() => vi.fn());
const fetchIcon = vi.hoisted(() => vi.fn());

const listInstalledApps = vi.hoisted(() => vi.fn());
const listUpdates = vi.hoisted(() => vi.fn().mockResolvedValue([]));
const getScoopConfig = vi.hoisted(() => vi.fn());
const getMirrorInfo = vi.hoisted(() => vi.fn());
const runScoop = vi.hoisted(() => vi.fn());
const getAppSettings = vi.hoisted(() => vi.fn().mockResolvedValue({ theme: "system" }));
const setTheme = vi.hoisted(() => vi.fn());
const otherCommands = vi.hoisted(() => ({
  detectRoots: vi.fn(),
  getActiveRoot: vi.fn(),
  setActiveRoot: vi.fn(),
  addRoot: vi.fn(),
  listBuckets: vi.fn(),
  cancelTask: vi.fn(),
  updatePackage: vi.fn(),
  uninstallPackage: vi.fn(),
  setHold: vi.fn(),
}));

vi.mock("../api/commands", () => ({
  ...otherCommands,
  listInstalledApps,
  listUpdates,
  getScoopConfig,
  getMirrorInfo,
  runScoop,
  fetchIcon,
  getAppSettings,
  setTheme,
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
  global: false,
};

const mirrorFixture: MirrorInfo = {
  scoop_repo: "https://gitee.com/scoop-installer/scoop",
  scoop_remote: "https://gitee.com/scoop-installer/scoop.git",
  presets: {
    official: "https://github.com/ScoopInstaller/scoop",
    gitee: "https://gitee.com/scoop-installer/scoop",
    proxy: "https://scoop.201704.xyz/https://github.com/ScoopInstaller/scoop",
  },
  url_proxy: null,
  default_url_proxy: "https://scoop.201704.xyz",
  proxy: null,
  buckets: [],
  aria2_installed: false,
  aria2_enabled: true,
  aria2_split: null,
  aria2_max_connection: null,
};

beforeEach(() => {
  setActivePinia(createPinia());
  fetchIcon.mockReset().mockResolvedValue(null);
  listUpdates.mockReset().mockResolvedValue([]);
  getMirrorInfo.mockReset().mockResolvedValue(mirrorFixture);
});

describe("AppIcon", () => {
  it("无 source 时显示首字母与固定背景色，且不请求图标", () => {
    const wrapper = mount(AppIcon, { props: { name: "git" } });
    expect(wrapper.text()).toContain("G");
    const style = wrapper.find(".icon").attributes("style") ?? "";
    expect(style.includes("#f0a020") || style.includes("rgb(240, 160, 32)")).toBe(true);
    expect(fetchIcon).not.toHaveBeenCalled();
  });

  it("source 抓取成功后用 img 展示图标", async () => {
    fetchIcon.mockResolvedValue("data:image/png;base64,AA==");
    const wrapper = mount(AppIcon, {
      props: { name: "git", source: "https://gitforwindows.org" },
    });
    expect(fetchIcon).toHaveBeenCalledWith("https://gitforwindows.org");
    await flushPromises();
    const img = wrapper.find("img");
    expect(img.exists()).toBe(true);
    expect(img.attributes("src")).toBe("data:image/png;base64,AA==");
    expect(wrapper.find(".icon").exists()).toBe(false);
  });

  it("source 抓取失败时回退首字母", async () => {
    fetchIcon.mockResolvedValue(null);
    const wrapper = mount(AppIcon, {
      props: { name: "7zip", source: "https://7-zip.org" },
    });
    await flushPromises();
    expect(wrapper.find("img").exists()).toBe(false);
    expect(wrapper.text()).toContain("7");
  });
});

describe("AppList", () => {
  it("渲染应用行并点击 emit select", async () => {
    const wrapper = mount(AppList, { props: { apps: [git], selected: null, updates: {} } });
    expect(wrapper.text()).toContain("git");
    expect(wrapper.text()).toContain("2.55.0.5");
    await wrapper.find(".row").trigger("click");
    expect(wrapper.emitted("select")?.[0]).toEqual(["git"]);
  });

  it("应用带 homepage 时列表展示抓取到的图标", async () => {
    fetchIcon.mockResolvedValue("data:image/png;base64,AA==");
    const wrapper = mount(AppList, { props: { apps: [git], selected: null, updates: {} } });
    await flushPromises();
    expect(fetchIcon).toHaveBeenCalledWith("https://gitforwindows.org");
    expect(wrapper.find("img").attributes("src")).toBe("data:image/png;base64,AA==");
  });

  it("有可更新时显示徽标", () => {
    const wrapper = mount(AppList, {
      props: {
        apps: [git],
        selected: null,
        updates: { git: { name: "git", current: "2.53.0.2", latest: "2.55.0.5", bucket: "main", hold: false, global: false } },
      },
    });
    expect(wrapper.text()).toContain("可更新");
  });

  it("全局应用显示徽标并按 key 选中", async () => {
    const globalGit: InstalledApp = { ...git, global: true };
    const wrapper = mount(AppList, {
      props: {
        apps: [globalGit],
        selected: "git@global",
        updates: {
          "git@global": { name: "git", current: "2.53.0.2", latest: "2.55.0.5", bucket: "main", hold: false, global: true },
        },
      },
    });
    expect(wrapper.text()).toContain("全局");
    expect(wrapper.text()).toContain("可更新");
    expect(wrapper.find(".row").classes()).toContain("active");
    await wrapper.find(".row").trigger("click");
    expect(wrapper.emitted("select")?.[0]).toEqual(["git@global"]);
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

  it("可更新时展示新版本提示", () => {
    const apps = useAppsStore();
    apps.updates = { git: { name: "git", current: "2.53.0.2", latest: "2.55.0.5", bucket: "main", hold: false, global: false } };
    const wrapper = mount(AppDetail, { props: { app: git } });
    expect(wrapper.text()).toContain("可更新至 2.55.0.5");
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

  it("镜像信息加载失败时展示错误并可重试", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const roots = useRootsStore();
    roots.active = {
      path: "C:\\scoop",
      valid: true,
      source: "env",
      is_active: true,
      app_count: 1,
      bucket_count: 1,
      has_portable_config: false,
      global_dir: null,
    };
    getScoopConfig.mockResolvedValue({
      file_path: "C:\\scoop\\config.json",
      portable: false,
      values: {},
    });
    getMirrorInfo.mockRejectedValueOnce("无法读取 scoop 仓库地址");
    const wrapper = mount(SettingsPage, { global: { plugins: [pinia] } });
    await flushPromises();
    expect(wrapper.text()).toContain("镜像信息加载失败");
    expect(wrapper.text()).toContain("无法读取 scoop 仓库地址");

    const retry = wrapper.findAll("button").find((b) => b.text().includes("重试"));
    await retry!.trigger("click");
    await flushPromises();
    expect(wrapper.text()).not.toContain("镜像信息加载失败");
    expect(wrapper.text()).toContain("镜像源");
  });

  it("镜像加载旧响应不覆盖新响应", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const roots = useRootsStore();
    const base = {
      path: "C:\\scoop",
      valid: true,
      source: "env",
      is_active: true,
      app_count: 1,
      bucket_count: 1,
      has_portable_config: false,
      global_dir: null,
    };
    roots.active = base;
    getScoopConfig.mockResolvedValue({
      file_path: "C:\\scoop\\config.json",
      portable: false,
      values: {},
    });
    let rejectFirst!: (e: unknown) => void;
    let resolveSecond!: (v: MirrorInfo) => void;
    getMirrorInfo
      .mockImplementationOnce(() => new Promise((_, rej) => (rejectFirst = rej)))
      .mockImplementationOnce(() => new Promise((res) => (resolveSecond = res)));
    const wrapper = mount(SettingsPage, { global: { plugins: [pinia] } });
    roots.active = { ...base, path: "D:\\scoop" };
    await flushPromises();
    resolveSecond(mirrorFixture);
    await flushPromises();
    expect(wrapper.text()).not.toContain("镜像信息加载失败");
    rejectFirst("旧请求失败");
    await flushPromises();
    expect(wrapper.text()).not.toContain("镜像信息加载失败");
    expect(wrapper.text()).toContain("镜像源");
  });

  it("切换外观调用 setTheme", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const roots = useRootsStore();
    roots.active = {
      path: "C:\\scoop",
      valid: true,
      source: "env",
      is_active: true,
      app_count: 1,
      bucket_count: 1,
      has_portable_config: false,
      global_dir: null,
    };
    getScoopConfig.mockResolvedValue({ file_path: "C:\\scoop\\config.json", portable: false, values: {} });
    getMirrorInfo.mockResolvedValue(mirrorFixture);
    const wrapper = mount(SettingsPage, { global: { plugins: [pinia] } });
    await flushPromises();
    wrapper.findComponent({ name: "RadioGroup" }).vm.$emit("update:value", "dark");
    await flushPromises();
    expect(setTheme).toHaveBeenCalledWith("dark");
  });
});
