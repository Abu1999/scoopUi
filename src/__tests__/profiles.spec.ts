import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  listProfiles: vi.fn(),
  getProfile: vi.fn(),
  profileDiff: vi.fn(),
  saveProfileFromCurrent: vi.fn(),
  importProfile: vi.fn(),
  exportProfile: vi.fn(),
  renameProfile: vi.fn(),
  deleteProfile: vi.fn(),
  applyProfileSettings: vi.fn(),
  bucketAdd: vi.fn(),
  installPackage: vi.fn(),
  uninstallPackage: vi.fn(),
  listInstalledApps: vi.fn(),
  listBuckets: vi.fn(),
  listUpdates: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { useLogsStore } from "../stores/logs";
import { useProfilesStore } from "../stores/profiles";
import type { Profile, ProfileDiff } from "../api/types";

const profileFixture: Profile = {
  version: 1,
  name: "工作",
  created: 1758614400,
  buckets: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
  apps: [{ name: "7zip", bucket: "main", global: false }],
  settings: { url_proxy: "https://scoop.201704.xyz" },
};

const emptyDiff: ProfileDiff = {
  missing_buckets: [],
  extra_buckets: [],
  bucket_source_conflicts: [],
  missing_apps: [],
  extra_apps: [],
  settings: [],
};

const diffFixture: ProfileDiff = {
  missing_buckets: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
  extra_buckets: [],
  bucket_source_conflicts: [],
  missing_apps: [{ name: "7zip", bucket: "main", global: false }],
  extra_apps: [],
  settings: [{ key: "url_proxy", current: null, target: "https://scoop.201704.xyz" }],
};

describe("profiles store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([]);
    mocks.listBuckets.mockResolvedValue([]);
    mocks.listUpdates.mockResolvedValue([]);
  });

  it("load 拉取列表并加载首个详情与差异", async () => {
    mocks.listProfiles.mockResolvedValue([
      { name: "工作", created: 1758614400, bucket_count: 1, app_count: 1, settings_count: 1 },
    ]);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue(emptyDiff);
    const store = useProfilesStore();
    await store.load();
    expect(store.selectedName).toBe("工作");
    expect(store.detail?.name).toBe("工作");
    expect(store.hasDiff).toBe(false);
    expect(store.loading).toBe(false);
  });

  it("load 失败时 error 可读且列表为空", async () => {
    mocks.listProfiles.mockRejectedValue("读取配置集目录失败");
    const store = useProfilesStore();
    await store.load();
    expect(store.list).toEqual([]);
    expect(store.error).toContain("读取配置集目录失败");
  });

  it("apply 依次执行 settings→桶→应用", async () => {
    mocks.profileDiff.mockResolvedValue(diffFixture);
    mocks.applyProfileSettings.mockResolvedValue(undefined);
    mocks.bucketAdd.mockResolvedValue(undefined);
    mocks.installPackage.mockResolvedValue(undefined);
    const store = useProfilesStore();
    store.selectedName = "工作";
    store.detail = profileFixture;
    const running = store.apply(false);
    await flushPromises();
    expect(mocks.applyProfileSettings).toHaveBeenCalledWith(profileFixture.settings);
    expect(mocks.bucketAdd).toHaveBeenCalledTimes(1);
    const bucketTask = mocks.bucketAdd.mock.calls[0][2];
    useLogsStore().end({ taskId: bucketTask, code: 0, success: true });
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalledWith("7zip", "main", null, false, expect.any(String));
    const installTask = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId: installTask, code: 0, success: true });
    await flushPromises();
    expect(await running).toBe(true);
    expect(store.steps.map((s) => s.status)).toEqual(["success", "success", "success"]);
    expect(mocks.listInstalledApps).toHaveBeenCalled();
  });

  it("apply 失败即停并标记剩余为待执行", async () => {
    mocks.profileDiff.mockResolvedValue({
      ...emptyDiff,
      missing_apps: [
        { name: "7zip", bucket: "main", global: false },
        { name: "git", bucket: "main", global: false },
      ],
    });
    mocks.installPackage.mockResolvedValue(undefined);
    const store = useProfilesStore();
    store.selectedName = "工作";
    store.detail = profileFixture;
    const running = store.apply(false);
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(await running).toBe(false);
    expect(store.steps[0].status).toBe("failed");
    expect(store.steps[1].status).toBe("skipped");
    expect(mocks.installPackage).toHaveBeenCalledTimes(1);
    expect(store.applyError).toContain("退出码 1");
  });

  it("saveFromCurrent 调用命令并刷新", async () => {
    mocks.saveProfileFromCurrent.mockResolvedValue(profileFixture);
    mocks.listProfiles.mockResolvedValue([
      { name: "工作", created: 1, bucket_count: 1, app_count: 1, settings_count: 1 },
    ]);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue(emptyDiff);
    const store = useProfilesStore();
    await store.saveFromCurrent("工作");
    expect(mocks.saveProfileFromCurrent).toHaveBeenCalledWith("工作");
    expect(store.selectedName).toBe("工作");
  });
});
