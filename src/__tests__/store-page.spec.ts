import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const messageError = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  detectRoots: vi.fn().mockResolvedValue([]),
  getActiveRoot: vi.fn().mockResolvedValue(null),
  setActiveRoot: vi.fn(),
  addRoot: vi.fn(),
  listInstalledApps: vi.fn().mockResolvedValue([]),
  listBuckets: vi.fn().mockResolvedValue([]),
  getScoopConfig: vi.fn(),
  runScoop: vi.fn(),
  cancelTask: vi.fn(),
  searchPackages: vi.fn(),
  refreshPackageIndex: vi.fn(),
  fetchIcon: vi.fn().mockResolvedValue(null),
  installPackage: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../api/commands", () => mocks);

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
  openUrl: vi.fn(() => Promise.resolve()),
  openPath: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import StorePage from "../pages/StorePage.vue";
import type { PackageEntry } from "../api/types";
import { useLogsStore } from "../stores/logs";
import { useRootsStore } from "../stores/roots";
import { useUiStore } from "../stores/ui";

function pkg(name: string, extra: Partial<PackageEntry> = {}): PackageEntry {
  return {
    name,
    bucket: "main",
    version: "1.0",
    description: null,
    homepage: null,
    license: null,
    icon: null,
    deprecated: false,
    ...extra,
  };
}

function list(n: number): PackageEntry[] {
  return Array.from({ length: n }, (_, i) => pkg(`p${String(i).padStart(3, "0")}`));
}

describe("StorePage", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.searchPackages.mockResolvedValue([
      pkg("git", { description: "分布式版本控制系统" }),
      pkg("git-lfs"),
    ]);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("挂载时执行一次搜索并渲染结果与详情", async () => {
    const wrapper = mount(StorePage);
    await flushPromises();
    expect(mocks.searchPackages).toHaveBeenCalledWith("", 50);
    expect(wrapper.text()).toContain("git-lfs");
    expect(wrapper.find(".detail").text()).toContain("分布式版本控制系统");
  });

  it("输入防抖 300ms 后按新关键词搜索", async () => {
    vi.useFakeTimers();
    const wrapper = mount(StorePage);
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.searchPackages).toHaveBeenCalledTimes(1);

    await wrapper.find("input").setValue("git");
    expect(mocks.searchPackages).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(299);
    expect(mocks.searchPackages).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1);
    expect(mocks.searchPackages).toHaveBeenLastCalledWith("git", 50);
  });

  it("可切换列表/卡片视图", async () => {
    const wrapper = mount(StorePage);
    await flushPromises();
    expect(wrapper.find(".results.list").exists()).toBe(true);
    const gridButton = wrapper.findAll("button").find((b) => b.text().includes("卡片"));
    expect(gridButton).toBeTruthy();
    await gridButton!.trigger("click");
    expect(wrapper.find(".results.grid").exists()).toBe(true);
  });

  it("点击结果切换详情", async () => {
    const wrapper = mount(StorePage);
    await flushPromises();
    const rows = wrapper.findAll(".row");
    await rows[1].trigger("click");
    expect(wrapper.find(".detail").text()).toContain("git-lfs");
  });

  it("刷新索引后重搜并显示包数量", async () => {
    mocks.refreshPackageIndex.mockResolvedValue(4955);
    const wrapper = mount(StorePage);
    await flushPromises();
    const button = wrapper.findAll("button").find((b) => b.text().includes("刷新索引"));
    expect(button).toBeTruthy();
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.refreshPackageIndex).toHaveBeenCalledTimes(1);
    expect(mocks.searchPackages).toHaveBeenCalledTimes(2);
    expect(wrapper.text()).toContain("4955");
  });

  it("根目录切换后按当前关键词重搜", async () => {
    mount(StorePage);
    await flushPromises();
    expect(mocks.searchPackages).toHaveBeenCalledTimes(1);
    const roots = useRootsStore();
    roots.active = {
      path: "D:\\scoop",
      valid: true,
      source: "manual",
      is_active: true,
      app_count: 0,
      bucket_count: 0,
      has_portable_config: false,
      global_dir: null,
    };
    await flushPromises();
    expect(mocks.searchPackages).toHaveBeenCalledTimes(2);
  });

  it("搜索失败展示错误文本", async () => {
    mocks.searchPackages.mockRejectedValue("未找到 scoop 根目录");
    const wrapper = mount(StorePage);
    await flushPromises();
    expect(wrapper.text()).toContain("未找到 scoop 根目录");
  });

  it("滚动接近底部时加载下一页", async () => {
    mocks.searchPackages.mockResolvedValueOnce(list(50));
    mocks.searchPackages.mockResolvedValueOnce(list(60));
    const wrapper = mount(StorePage);
    await flushPromises();
    const scroller = wrapper.find(".results").element as HTMLElement;
    Object.defineProperty(scroller, "scrollHeight", { value: 1000, configurable: true });
    Object.defineProperty(scroller, "clientHeight", { value: 400, configurable: true });
    Object.defineProperty(scroller, "scrollTop", { value: 500, configurable: true, writable: true });
    await wrapper.find(".results").trigger("scroll");
    await flushPromises();
    expect(mocks.searchPackages).toHaveBeenLastCalledWith("", 100);
  });

  it("不足以滚动时切换卡片视图会自动继续加载", async () => {
    mocks.searchPackages.mockResolvedValueOnce(list(50));
    mocks.searchPackages.mockResolvedValueOnce(list(60));
    const wrapper = mount(StorePage);
    await flushPromises();
    expect(mocks.searchPackages).toHaveBeenCalledTimes(1);
    const scroller = wrapper.find(".results").element as HTMLElement;
    Object.defineProperty(scroller, "clientHeight", { value: 600, configurable: true });
    Object.defineProperty(scroller, "scrollHeight", { value: 500, configurable: true });
    const gridButton = wrapper.findAll("button").find((b) => b.text().includes("卡片"));
    await gridButton!.trigger("click");
    await flushPromises();
    expect(mocks.searchPackages).toHaveBeenLastCalledWith("", 100);
  });

  it("距底部尚远时不触发加载下一页", async () => {
    mocks.searchPackages.mockResolvedValueOnce(list(50));
    const wrapper = mount(StorePage);
    await flushPromises();
    const scroller = wrapper.find(".results").element as HTMLElement;
    Object.defineProperty(scroller, "scrollHeight", { value: 1000, configurable: true });
    Object.defineProperty(scroller, "clientHeight", { value: 400, configurable: true });
    Object.defineProperty(scroller, "scrollTop", { value: 100, configurable: true, writable: true });
    await wrapper.find(".results").trigger("scroll");
    await flushPromises();
    expect(mocks.searchPackages).toHaveBeenCalledTimes(1);
  });

  it("结果少于页大小时显示已全部加载", async () => {
    mocks.searchPackages.mockResolvedValue([pkg("git"), pkg("git-lfs")]);
    const wrapper = mount(StorePage);
    await flushPromises();
    expect(wrapper.text()).toContain("已显示全部 2 条");
  });

  it("安装按钮调用 installPackage 并在成功后刷新已安装列表", async () => {
    mocks.searchPackages.mockResolvedValue([pkg("git", { description: "git" })]);
    const wrapper = mount(StorePage);
    await flushPromises();
    await wrapper.find('[role="checkbox"]').trigger("click");
    const button = wrapper.findAll("button").find((b) => b.text().includes("安装"));
    expect(button).toBeTruthy();
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalledWith("git", "main", null, false, expect.any(String));
    const taskId = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(mocks.listInstalledApps).toHaveBeenCalled();
    expect(mocks.detectRoots).toHaveBeenCalled();
  });

  it("全局安装默认勾选", async () => {
    mocks.searchPackages.mockResolvedValue([pkg("git")]);
    const wrapper = mount(StorePage);
    await flushPromises();
    expect(wrapper.find('[role="checkbox"]').classes()).toContain("n-checkbox--checked");
  });

  it("版本与全局选项传入安装命令", async () => {
    mocks.searchPackages.mockResolvedValue([pkg("git")]);
    const wrapper = mount(StorePage);
    await flushPromises();
    await wrapper.find('input[placeholder="版本（留空 = 最新）"]').setValue("2.7.0");
    const button = wrapper.findAll("button").find((b) => b.text().includes("安装"));
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalledWith("git", "main", "2.7.0", true, expect.any(String));
  });

  it("同一时刻重复点击安装只发起一次", async () => {
    const wrapper = mount(StorePage);
    await flushPromises();
    const button = wrapper.findAll("button").find((b) => b.text().includes("安装"));
    expect(button).toBeTruthy();
    const first = button!.trigger("click");
    const second = button!.trigger("click");
    await first;
    await second;
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalledTimes(1);
  });

  it("安装失败提示错误", async () => {
    mocks.searchPackages.mockResolvedValue([pkg("git")]);
    const wrapper = mount(StorePage);
    await flushPromises();
    const button = wrapper.findAll("button").find((b) => b.text().includes("安装"));
    await button!.trigger("click");
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalledWith("安装失败（退出码 1），详见任务日志");
  });

  it("消费来自迁移页的搜索意图", async () => {
    useUiStore().storeSearchQuery = "7zip";
    mount(StorePage);
    await flushPromises();
    expect(mocks.searchPackages).toHaveBeenCalledWith("7zip", 50);
    expect(useUiStore().storeSearchQuery).toBeNull();
  });
});
