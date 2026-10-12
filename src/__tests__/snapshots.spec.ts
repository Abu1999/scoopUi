import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

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

import { useLogsStore } from "../stores/logs";
import { useSnapshotsStore } from "../stores/snapshots";
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
  missing_apps: [{ name: "7zip", bucket: "main", version: "26.00", hold: false, global: false }],
};

describe("snapshots store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([]);
    mocks.listBuckets.mockResolvedValue([]);
    mocks.listUpdates.mockResolvedValue([]);
  });

  it("load 拉取列表并加载计划", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    const store = useSnapshotsStore();
    await store.load();
    expect(store.selectedId).toBe("100");
    expect(store.selected?.root).toBe("C:\\scoop");
    expect(store.plan?.missing_apps[0].name).toBe("7zip");
    expect(store.loading).toBe(false);
  });

  it("load 失败时 error 可读且列表为空", async () => {
    mocks.listSnapshots.mockRejectedValue("读取快照目录失败");
    const store = useSnapshotsStore();
    await store.load();
    expect(store.list).toEqual([]);
    expect(store.error).toContain("读取快照目录失败");
  });

  it("restoreConfigAndBuckets 恢复配置并添加缺失软件源", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    mocks.restoreSnapshotConfig.mockResolvedValue(undefined);
    mocks.restoreSnapshotBuckets.mockResolvedValue({
      reset: ["main"],
      missing: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
      skipped: [],
    });
    mocks.bucketAdd.mockResolvedValue(undefined);
    const store = useSnapshotsStore();
    await store.load();
    const running = store.restoreConfigAndBuckets();
    await flushPromises();
    expect(mocks.restoreSnapshotConfig).toHaveBeenCalledWith("100");
    expect(mocks.restoreSnapshotBuckets).toHaveBeenCalledWith("100");
    expect(mocks.bucketAdd).toHaveBeenCalledWith(
      "extras",
      "https://gitee.com/scoop-installer/Extras",
      expect.any(String),
    );
    const taskId = mocks.bucketAdd.mock.calls[0][2];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(await running).toBe(true);
    expect(store.steps.map((s) => s.status)).toEqual(["success"]);
  });

  it("installAllMissing 失败即停并标记剩余", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValue({
      ...planFixture,
      missing_apps: [
        { name: "7zip", bucket: "main", version: "26.00", hold: false, global: false },
        { name: "git", bucket: "main", version: "2.55.0.5", hold: false, global: false },
      ],
    });
    mocks.installPackage.mockResolvedValue(undefined);
    const store = useSnapshotsStore();
    await store.load();
    const running = store.installAllMissing();
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(await running).toBe(false);
    expect(store.steps[0].status).toBe("failed");
    expect(store.steps[1].status).toBe("skipped");
    expect(mocks.installPackage).toHaveBeenCalledTimes(1);
    expect(store.restoreError).toContain("退出码 1");
  });

  it("create 后选中新快照并刷新", async () => {
    mocks.createSnapshot.mockResolvedValue({ ...summary, id: "200", created: 1758614500 });
    mocks.listSnapshots.mockResolvedValue([
      { ...summary, id: "200", created: 1758614500 },
      summary,
    ]);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    const store = useSnapshotsStore();
    await store.create();
    expect(mocks.createSnapshot).toHaveBeenCalled();
    expect(store.selectedId).toBe("200");
  });

  it("无配置快照跳过配置恢复但仍恢复软件源", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValue({ ...planFixture, config_present: false });
    mocks.restoreSnapshotBuckets.mockResolvedValue({
      reset: [],
      missing: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
      skipped: [],
    });
    mocks.bucketAdd.mockResolvedValue(undefined);
    const store = useSnapshotsStore();
    await store.load();
    const running = store.restoreConfigAndBuckets();
    await flushPromises();
    expect(mocks.restoreSnapshotConfig).not.toHaveBeenCalled();
    expect(mocks.restoreSnapshotBuckets).toHaveBeenCalledWith("100");
    const taskId = mocks.bucketAdd.mock.calls[0][2];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(await running).toBe(true);
  });

  it("select 切换失败时清空旧计划并记错", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValueOnce(planFixture).mockRejectedValueOnce("计划计算失败");
    const store = useSnapshotsStore();
    await store.load();
    expect(store.plan).not.toBeNull();
    await store.select("100");
    expect(store.plan).toBeNull();
    expect(store.error).toContain("计划计算失败");
  });
});
