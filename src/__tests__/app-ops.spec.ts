import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { NPopconfirm } from "naive-ui";

const messageError = vi.hoisted(() => vi.fn());
const messageSuccess = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  listInstalledApps: vi.fn(),
  updatePackage: vi.fn(),
  uninstallPackage: vi.fn(),
  forceUninstallPackage: vi.fn(),
  listAppProcesses: vi.fn().mockResolvedValue([]),
  setHold: vi.fn(),
  fetchIcon: vi.fn().mockResolvedValue(null),
  getScoopEnv: vi.fn().mockResolvedValue(null),
  listUpdates: vi.fn().mockResolvedValue([]),
  getMirrorInfo: vi.fn().mockResolvedValue(null),
}));
vi.mock("../api/commands", () => mocks);

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: messageError,
      success: messageSuccess,
      info: vi.fn(),
      warning: vi.fn(),
    }),
  };
});

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: vi.fn(() => Promise.resolve()),
  openUrl: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import AppDetail from "../components/app/AppDetail.vue";
import { useAppsStore } from "../stores/apps";
import { useLogsStore } from "../stores/logs";
import type { InstalledApp } from "../api/types";

const git: InstalledApp = {
  name: "git",
  version: "2.55.0.5",
  bucket: "main",
  architecture: "64bit",
  hold: false,
  updated: null,
  description: "git",
  homepage: null,
  icon: null,
  depends: [],
  persist: [],
  bin: ["git"],
  app_dir: "C:\\scoop\\apps\\git",
};

function findButton(wrapper: ReturnType<typeof mount>, text: string) {
  const button = wrapper.findAll("button").find((b) => b.text().includes(text));
  expect(button).toBeTruthy();
  return button!;
}

describe("AppDetail 操作", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([git]);
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("更新成功后刷新已安装列表", async () => {
    mocks.updatePackage.mockResolvedValue(undefined);
    const wrapper = mount(AppDetail, { props: { app: git } });
    await findButton(wrapper, "更新").trigger("click");
    await flushPromises();
    expect(mocks.updatePackage).toHaveBeenCalledWith("git", false, expect.any(String));
    const taskId = mocks.updatePackage.mock.calls[0][2];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(mocks.listInstalledApps).toHaveBeenCalledTimes(1);
    expect(useAppsStore().apps).toEqual([git]);
    expect(mocks.getScoopEnv).toHaveBeenCalled();
  });

  it("卸载失败提示错误且不刷新", async () => {
    mocks.uninstallPackage.mockResolvedValue(undefined);
    const wrapper = mount(AppDetail, { props: { app: git } });
    await wrapper.findComponent(NPopconfirm).vm.$emit("positive-click");
    await flushPromises();
    expect(mocks.uninstallPackage).toHaveBeenCalledWith("git", false, expect.any(String));
    const taskId = mocks.uninstallPackage.mock.calls[0][2];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalledWith("操作失败（退出码 1），详见任务日志");
    expect(mocks.listInstalledApps).not.toHaveBeenCalled();
  });

  it("锁定调用 setHold(hold=true)", async () => {
    mocks.setHold.mockResolvedValue(undefined);
    const wrapper = mount(AppDetail, { props: { app: git } });
    await findButton(wrapper, "锁定").trigger("click");
    await flushPromises();
    expect(mocks.setHold).toHaveBeenCalledWith("git", true, expect.any(String));
  });

  it("任务运行中应用操作按钮不再禁用（并发冲突由后端校验）", () => {
    useLogsStore().start("t1", "安装 python");
    const wrapper = mount(AppDetail, { props: { app: git } });
    expect(findButton(wrapper, "更新").attributes("disabled")).toBeUndefined();
    expect(findButton(wrapper, "卸载").attributes("disabled")).toBeUndefined();
    expect(findButton(wrapper, "锁定").attributes("disabled")).toBeUndefined();
  });

  it("任务运行中确认卸载仍会发起（并发冲突由后端拒绝）", async () => {
    mocks.uninstallPackage.mockResolvedValue(undefined);
    const wrapper = mount(AppDetail, { props: { app: git } });
    useLogsStore().start("t1", "安装 python");
    await wrapper.findComponent(NPopconfirm).vm.$emit("positive-click");
    await flushPromises();
    expect(mocks.uninstallPackage).toHaveBeenCalledWith("git", false, expect.any(String));
    const taskId = mocks.uninstallPackage.mock.calls[0][2];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
  });

  it("卸载后应用仍存在时弹出强制卸载并调用 forceUninstallPackage", async () => {
    mocks.uninstallPackage.mockResolvedValue(undefined);
    mocks.listInstalledApps.mockResolvedValue([git]);
    mocks.listAppProcesses.mockResolvedValue([{ pid: 321, name: "git" }]);
    mocks.forceUninstallPackage.mockResolvedValue(undefined);
    const wrapper = mount(AppDetail, { props: { app: git }, attachTo: document.body });
    await wrapper.findComponent(NPopconfirm).vm.$emit("positive-click");
    await flushPromises();
    const taskId = mocks.uninstallPackage.mock.calls[0][2];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();

    expect(mocks.listAppProcesses).toHaveBeenCalledWith("git");
    expect(document.body.textContent).toContain("应用未能完全卸载");
    expect(document.body.textContent).toContain("git（PID 321）");

    const forceButton = Array.from(document.body.querySelectorAll("button")).find((b) =>
      b.textContent?.includes("强制卸载"),
    ) as HTMLButtonElement;
    expect(forceButton).toBeTruthy();
    mocks.listInstalledApps.mockResolvedValue([]);
    forceButton.click();
    await flushPromises();
    expect(mocks.forceUninstallPackage).toHaveBeenCalledWith("git", false, expect.any(String));

    const forceTaskId = mocks.forceUninstallPackage.mock.calls[0][2];
    useLogsStore().end({ taskId: forceTaskId, code: 0, success: true });
    await flushPromises();
    expect(messageSuccess).toHaveBeenCalledWith("已强制卸载 git");
    wrapper.unmount();
  });
});
