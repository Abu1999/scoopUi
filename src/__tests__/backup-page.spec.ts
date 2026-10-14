import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const messageError = vi.hoisted(() => vi.fn());
const messageSuccess = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  listSnapshots: vi.fn(),
  createSnapshot: vi.fn(),
  deleteSnapshot: vi.fn(),
  restoreSnapshotPlan: vi.fn(),
  restoreSnapshotConfig: vi.fn(),
  restoreSnapshotBuckets: vi.fn(),
  bucketAdd: vi.fn(),
  installPackage: vi.fn(),
  listInstalledApps: vi.fn(),
  listBuckets: vi.fn(),
  listUpdates: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);
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
      info: vi.fn(),
      warning: vi.fn(),
    }),
  };
});

import BackupPage from "../pages/BackupPage.vue";
import { useLogsStore } from "../stores/logs";
import type { RestorePlan } from "../api/types";

const summary = {
  id: "100",
  created: 1758614400,
  root: "C:\\scoop",
  app_count: 2,
  bucket_count: 1,
  profile_count: 1,
};

const planFixture: RestorePlan = {
  root: "C:\\scoop",
  root_matches: true,
  config_path: "C:\\scoop\\config.json",
  config_present: true,
  reset_buckets: [{ name: "main", source: "https://gitee.com/scoop-installer/Main" }],
  missing_buckets: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
  missing_apps: [{ name: "7zip", bucket: "main", version: "26.00", hold: false }],
};

describe("BackupPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([]);
    mocks.listBuckets.mockResolvedValue([]);
    mocks.listUpdates.mockResolvedValue([]);
  });

  it("首次挂载自动加载列表并渲染恢复计划", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mount(BackupPage, { global: { plugins: [pinia] } });
    await flushPromises();
    expect(mocks.listSnapshots).toHaveBeenCalled();
    expect(wrapper.text()).toContain("C:\\scoop");
    expect(wrapper.text()).toContain("缺失应用");
    expect(wrapper.text()).toContain("7zip");
    expect(wrapper.text()).toContain("缺失软件源");
  });

  it("创建快照调用命令", async () => {
    mocks.listSnapshots.mockResolvedValue([]);
    mocks.createSnapshot.mockResolvedValue(summary);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mount(BackupPage, { global: { plugins: [pinia] } });
    await flushPromises();
    const btn = wrapper.findAll("button").find((b) => b.text().includes("创建快照"));
    await btn!.trigger("click");
    await flushPromises();
    expect(mocks.createSnapshot).toHaveBeenCalled();
    expect(messageSuccess).toHaveBeenCalled();
  });

  it("恢复配置与软件源调用命令并添加缺失源", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    mocks.restoreSnapshotConfig.mockResolvedValue(undefined);
    mocks.restoreSnapshotBuckets.mockResolvedValue({
      reset: ["main"],
      missing: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
      skipped: [],
    });
    mocks.bucketAdd.mockResolvedValue(undefined);
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mount(BackupPage, { global: { plugins: [pinia] } });
    await flushPromises();
    const btn = wrapper.findAll("button").find((b) => b.text() === "恢复配置与软件源");
    await btn!.trigger("click");
    await flushPromises();
    expect(mocks.restoreSnapshotConfig).toHaveBeenCalledWith("100");
    expect(mocks.bucketAdd).toHaveBeenCalledWith(
      "extras",
      "https://gitee.com/scoop-installer/Extras",
      expect.any(String),
    );
    const taskId = mocks.bucketAdd.mock.calls[0][2];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(messageSuccess).toHaveBeenCalled();
  });

  it("缺失应用安装失败提示错误", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    mocks.installPackage.mockResolvedValue(undefined);
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mount(BackupPage, { global: { plugins: [pinia] } });
    await flushPromises();
    const install = wrapper.findAll("button").find((b) => b.text() === "安装");
    await install!.trigger("click");
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][3];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalled();
  });
});
