import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const messageError = vi.hoisted(() => vi.fn());
const messageSuccess = vi.hoisted(() => vi.fn());
const messageWarning = vi.hoisted(() => vi.fn());
const messageInfo = vi.hoisted(() => vi.fn());
const dialogOpen = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  checkScoopPrereqs: vi.fn(),
  getScoopBundleInfo: vi.fn(),
  extractScoopBundle: vi.fn(),
  finalizeScoopBootstrap: vi.fn(),
  installScoopOnline: vi.fn(),
  adoptScoopDir: vi.fn(),
  getScoopEnv: vi.fn(),
  listInstalledApps: vi.fn(),
  listBuckets: vi.fn(),
  listUpdates: vi.fn(),
  fetchIcon: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: dialogOpen }));
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
      info: messageInfo,
      warning: messageWarning,
    }),
  };
});

import SetupPage from "../pages/SetupPage.vue";
import { useLogsStore } from "../stores/logs";
import { useUiStore } from "../stores/ui";
import type { PrereqReport } from "../api/types";

const report: PrereqReport = {
  default_dir: "C:\\Users\\Administrator\\scoop",
  dir: "C:\\Users\\Administrator\\scoop",
  dir_ok: true,
  dir_reason: null,
  powershell_ok: true,
  tar_available: true,
  git_available: true,
  elevated: false,
  existing_root: null,
};

const bundle = {
  built: "2026-10-13 16:50:07",
  scoop_commit: "ce867cccb6d4e7f7ad5cee8b57bc5b9b94cd7840",
  main_commit: "6686d10590e5de4768ca5e2dcd9cfa89020779bb",
};

const envFixture = {
  path: "C:\\Users\\Administrator\\scoop",
  valid: true,
  app_count: 1679,
  bucket_count: 1,
  has_portable_config: false,
};

function findButton(wrapper: ReturnType<typeof mount>, text: string) {
  const btn = wrapper.findAll("button").find((b) => b.text().includes(text));
  expect(btn).toBeTruthy();
  return btn!;
}

describe("SetupPage", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.checkScoopPrereqs.mockResolvedValue(report);
    mocks.getScoopBundleInfo.mockResolvedValue(bundle);
    mocks.adoptScoopDir.mockResolvedValue(envFixture);
    mocks.getScoopEnv.mockResolvedValue(envFixture);
    mocks.listInstalledApps.mockResolvedValue([]);
    mocks.listBuckets.mockResolvedValue([]);
    mocks.listUpdates.mockResolvedValue([]);
    mocks.fetchIcon.mockResolvedValue(null);
    mocks.finalizeScoopBootstrap.mockResolvedValue({
      manifest_count: 1679,
      path_updated: true,
      warnings: [],
    });
  });

  it("渲染环境卡与快照信息", async () => {
    const wrapper = mount(SetupPage);
    await flushPromises();
    expect(wrapper.text()).toContain("内置快照");
    expect(wrapper.text()).toContain("ce867cc");
    expect(wrapper.text()).toContain("PowerShell");
    expect(wrapper.text()).toContain("一键初始化");
  });

  it("一键初始化成功链路", async () => {
    mocks.extractScoopBundle.mockResolvedValue(undefined);
    const wrapper = mount(SetupPage);
    await flushPromises();
    await findButton(wrapper, "一键初始化").trigger("click");
    await flushPromises();
    expect(mocks.extractScoopBundle).toHaveBeenCalledWith(
      "C:\\Users\\Administrator\\scoop",
      expect.any(String),
    );
    const taskId = mocks.extractScoopBundle.mock.calls[0][1];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(mocks.finalizeScoopBootstrap).toHaveBeenCalledWith("C:\\Users\\Administrator\\scoop", true);
    expect(mocks.adoptScoopDir).toHaveBeenCalledWith("C:\\Users\\Administrator\\scoop");
    expect(messageSuccess).toHaveBeenCalledWith("Scoop 已就绪");
    expect(useUiStore().activePage).toBe("installed");
  });

  it("快速连点初始化只执行一次", async () => {
    mocks.extractScoopBundle.mockResolvedValue(undefined);
    const wrapper = mount(SetupPage);
    await flushPromises();
    const btn = findButton(wrapper, "一键初始化");
    const first = btn.trigger("click");
    const second = btn.trigger("click");
    await first;
    await second;
    await flushPromises();
    expect(mocks.extractScoopBundle).toHaveBeenCalledTimes(1);
  });

  it("解压失败提示错误且不继续", async () => {
    mocks.extractScoopBundle.mockResolvedValue(undefined);
    const wrapper = mount(SetupPage);
    await flushPromises();
    await findButton(wrapper, "一键初始化").trigger("click");
    await flushPromises();
    const taskId = mocks.extractScoopBundle.mock.calls[0][1];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalled();
    expect(mocks.finalizeScoopBootstrap).not.toHaveBeenCalled();
    expect(mocks.adoptScoopDir).not.toHaveBeenCalled();
  });

  it("选择目录更新目标路径并重新检查", async () => {
    dialogOpen.mockResolvedValue("D:\\scoop-test");
    const wrapper = mount(SetupPage);
    await flushPromises();
    await findButton(wrapper, "选择").trigger("click");
    await flushPromises();
    expect(mocks.checkScoopPrereqs).toHaveBeenLastCalledWith("D:\\scoop-test");
  });

  it("所选目录非空时自动改用子目录", async () => {
    dialogOpen.mockResolvedValue("D:\\");
    const wrapper = mount(SetupPage);
    await flushPromises();
    mocks.checkScoopPrereqs
      .mockResolvedValueOnce({ ...report, dir: "D:\\", dir_ok: false, dir_reason: "目标目录非空: D:\\" })
      .mockResolvedValue({ ...report, dir: "D:\\scoop" });
    await findButton(wrapper, "选择").trigger("click");
    await flushPromises();
    expect(messageInfo).toHaveBeenCalled();
    expect(mocks.checkScoopPrereqs).toHaveBeenLastCalledWith("D:\\scoop");
  });

  it("finalize 警告时提示但仍完成接入", async () => {
    mocks.extractScoopBundle.mockResolvedValue(undefined);
    mocks.finalizeScoopBootstrap.mockResolvedValue({
      manifest_count: 1679,
      path_updated: false,
      warnings: ["加入 PATH 失败: 拒绝访问"],
    });
    const wrapper = mount(SetupPage);
    await flushPromises();
    await findButton(wrapper, "一键初始化").trigger("click");
    await flushPromises();
    const taskId = mocks.extractScoopBundle.mock.calls[0][1];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(messageWarning).toHaveBeenCalled();
    expect(mocks.adoptScoopDir).toHaveBeenCalled();
    expect(messageSuccess).toHaveBeenCalledWith("Scoop 已就绪");
  });

  it("在线安装入口调用命令", async () => {
    mocks.installScoopOnline.mockResolvedValue(undefined);
    const wrapper = mount(SetupPage);
    await flushPromises();
    await findButton(wrapper, "在线安装").trigger("click");
    await flushPromises();
    expect(mocks.installScoopOnline).toHaveBeenCalledWith(
      "C:\\Users\\Administrator\\scoop",
      null,
      expect.any(String),
    );
  });
});
