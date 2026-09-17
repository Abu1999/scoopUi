import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  detectRoots: vi.fn(),
  getActiveRoot: vi.fn(),
  setActiveRoot: vi.fn(),
  addRoot: vi.fn(),
  listInstalledApps: vi.fn(),
  listBuckets: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);

import { useAppsStore } from "../stores/apps";
import { useBucketsStore } from "../stores/buckets";
import { useRootsStore } from "../stores/roots";

const rootFixture = {
  path: "C:\\scoop",
  valid: true,
  source: "default",
  is_active: true,
  app_count: 9,
  bucket_count: 4,
  has_portable_config: false,
  global_dir: null,
};

const appFixture = {
  name: "git",
  version: "2.55.0.5",
  bucket: "main",
  architecture: "64bit",
  hold: false,
  updated: null,
  description: "git",
  homepage: "https://gitforwindows.org",
  icon: null,
  depends: [],
  persist: [],
  bin: ["git"],
  app_dir: "C:\\scoop\\apps\\git",
};

describe("stores", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("roots.load 同时拉取 active 与候选", async () => {
    mocks.getActiveRoot.mockResolvedValue(rootFixture);
    mocks.detectRoots.mockResolvedValue([rootFixture]);
    const store = useRootsStore();
    await store.load();
    expect(store.active?.path).toBe("C:\\scoop");
    expect(store.roots).toHaveLength(1);
    expect(store.loading).toBe(false);
  });

  it("roots.load 失败时 error 为可读字符串", async () => {
    mocks.getActiveRoot.mockRejectedValue("未找到可用的 scoop 根目录");
    mocks.detectRoots.mockResolvedValue([]);
    const store = useRootsStore();
    await store.load();
    expect(store.error).toBe("未找到可用的 scoop 根目录");
    expect(store.active).toBeNull();
  });

  it("roots.setActive 调用命令并刷新", async () => {
    mocks.setActiveRoot.mockResolvedValue(rootFixture);
    mocks.getActiveRoot.mockResolvedValue(rootFixture);
    mocks.detectRoots.mockResolvedValue([rootFixture]);
    const store = useRootsStore();
    await store.setActive("C:\\scoop");
    expect(mocks.setActiveRoot).toHaveBeenCalledWith("C:\\scoop");
    expect(store.active?.path).toBe("C:\\scoop");
  });

  it("roots.setActive 会刷新根目录", async () => {
    mocks.getActiveRoot.mockResolvedValue(rootFixture);
    mocks.detectRoots.mockResolvedValue([rootFixture]);
    mocks.setActiveRoot.mockResolvedValue(rootFixture);
    const store = useRootsStore();
    await store.load();
    await store.setActive("C:\\scoop");
    expect(mocks.detectRoots).toHaveBeenCalled();
    expect(mocks.getActiveRoot).toHaveBeenCalledTimes(2);
  });

  it("roots.add 失败时写入 error", async () => {
    mocks.addRoot.mockRejectedValue("根目录无效（缺少 apps/buckets/shims 目录）: X");
    const store = useRootsStore();
    await store.add("X");
    expect(store.error).toContain("根目录无效（缺少 apps/buckets/shims 目录）: X");
  });

  it("apps.load 拉取列表并支持选中", async () => {
    mocks.listInstalledApps.mockResolvedValue([appFixture]);
    const store = useAppsStore();
    await store.load();
    store.select("git");
    expect(store.apps).toHaveLength(1);
    expect(store.selected?.name).toBe("git");
  });

  it("apps.load 选中在列表变化时回退", async () => {
    mocks.listInstalledApps.mockResolvedValue([appFixture]);
    const store = useAppsStore();
    await store.load();
    expect(store.selectedName).toBe("git");

    mocks.listInstalledApps.mockResolvedValue([]);
    await store.load();
    expect(store.selectedName).toBeNull();

    mocks.listInstalledApps.mockResolvedValue([{ ...appFixture, name: "7zip" }]);
    store.select("git");
    await store.load();
    expect(store.selectedName).toBe("7zip");
  });

  it("apps.load 失败时 error 与空列表", async () => {
    mocks.listInstalledApps.mockRejectedValue("未找到可用的 scoop 根目录");
    const store = useAppsStore();
    await store.load();
    expect(store.apps).toEqual([]);
    expect(store.error).toContain("未找到");
  });

  it("buckets.load 拉取列表", async () => {
    mocks.listBuckets.mockResolvedValue([{ name: "main", source: "https://x", is_git: true, manifest_count: 1641 }]);
    const store = useBucketsStore();
    await store.load();
    expect(store.buckets[0]).toMatchObject({ name: "main", manifest_count: 1641 });
  });

  it("buckets.load 失败时 error 与空列表", async () => {
    mocks.listBuckets.mockRejectedValue("未找到可用的 scoop 根目录");
    const store = useBucketsStore();
    await store.load();
    expect(store.buckets).toEqual([]);
    expect(store.error).toContain("未找到");
  });
});
