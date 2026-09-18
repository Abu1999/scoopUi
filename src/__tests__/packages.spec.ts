import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  searchPackages: vi.fn(),
  refreshPackageIndex: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);

import type { PackageEntry } from "../api/types";
import { usePackagesStore } from "../stores/packages";

function pkg(name: string): PackageEntry {
  return {
    name,
    bucket: "main",
    version: "1.0",
    description: null,
    homepage: null,
    license: null,
    icon: null,
    deprecated: false,
  };
}

function list(n: number): PackageEntry[] {
  return Array.from({ length: n }, (_, i) => pkg(`p${String(i).padStart(3, "0")}`));
}

describe("packages store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("search 拉取结果并默认选中第一个", async () => {
    mocks.searchPackages.mockResolvedValue([pkg("git"), pkg("git-lfs")]);
    const store = usePackagesStore();
    await store.search("git");
    expect(mocks.searchPackages).toHaveBeenCalledWith("git", 50);
    expect(store.results).toHaveLength(2);
    expect(store.selectedName).toBe("git");
    expect(store.selected?.name).toBe("git");
    expect(store.loading).toBe(false);
  });

  it("选中项在结果变化时回退到第一个", async () => {
    mocks.searchPackages.mockResolvedValueOnce([pkg("git"), pkg("git-lfs")]);
    const store = usePackagesStore();
    await store.search("git");
    store.select("git-lfs");
    mocks.searchPackages.mockResolvedValueOnce([pkg("git")]);
    await store.search("git");
    expect(store.selectedName).toBe("git");
  });

  it("慢的旧响应不会覆盖新结果", async () => {
    let resolveFirst!: (value: PackageEntry[]) => void;
    mocks.searchPackages.mockImplementationOnce(
      () =>
        new Promise<PackageEntry[]>((resolve) => {
          resolveFirst = resolve;
        }),
    );
    mocks.searchPackages.mockResolvedValueOnce([pkg("new")]);
    const store = usePackagesStore();
    const first = store.search("old");
    await store.search("new");
    resolveFirst([pkg("stale")]);
    await first;
    expect(store.query).toBe("new");
    expect(store.results.map((p) => p.name)).toEqual(["new"]);
  });

  it("search 失败写入 error 并清空结果", async () => {
    mocks.searchPackages.mockRejectedValue("未找到 scoop 根目录");
    const store = usePackagesStore();
    await store.search("git");
    expect(store.error).toBe("未找到 scoop 根目录");
    expect(store.results).toEqual([]);
  });

  it("refresh 重建索引后按当前查询重搜并记录数量", async () => {
    mocks.searchPackages.mockResolvedValue([pkg("git")]);
    const store = usePackagesStore();
    await store.search("git");
    mocks.refreshPackageIndex.mockResolvedValue(4955);
    await store.refresh();
    expect(mocks.refreshPackageIndex).toHaveBeenCalledTimes(1);
    expect(mocks.searchPackages).toHaveBeenCalledTimes(2);
    expect(mocks.searchPackages).toHaveBeenLastCalledWith("git", 50);
    expect(store.indexCount).toBe(4955);
    expect(store.refreshing).toBe(false);
  });

  it("loadMore 增大 limit 并保留已加载结果与选中项", async () => {
    mocks.searchPackages.mockResolvedValueOnce(list(50));
    const store = usePackagesStore();
    await store.search("");
    expect(store.limit).toBe(50);
    expect(store.loadedAll).toBe(false);
    store.select("p049");
    mocks.searchPackages.mockResolvedValueOnce(list(60));
    await store.loadMore();
    expect(mocks.searchPackages).toHaveBeenLastCalledWith("", 100);
    expect(store.results).toHaveLength(60);
    expect(store.loadedAll).toBe(true);
    expect(store.selectedName).toBe("p049");
  });

  it("结果少于页大小视为已全部加载，loadMore 不再请求", async () => {
    mocks.searchPackages.mockResolvedValue([pkg("git"), pkg("git-lfs")]);
    const store = usePackagesStore();
    await store.search("git");
    expect(store.loadedAll).toBe(true);
    await store.loadMore();
    expect(mocks.searchPackages).toHaveBeenCalledTimes(1);
  });

  it("新搜索重置 limit 为 50", async () => {
    mocks.searchPackages.mockResolvedValueOnce(list(50));
    const store = usePackagesStore();
    await store.search("");
    mocks.searchPackages.mockResolvedValueOnce(list(60));
    await store.loadMore();
    mocks.searchPackages.mockResolvedValueOnce([pkg("x")]);
    await store.search("x");
    expect(mocks.searchPackages).toHaveBeenLastCalledWith("x", 50);
    expect(store.limit).toBe(50);
  });

  it("loadMore 失败保留现有结果与错误信息", async () => {
    mocks.searchPackages.mockResolvedValueOnce(list(50));
    const store = usePackagesStore();
    await store.search("");
    mocks.searchPackages.mockRejectedValueOnce("未找到 scoop 根目录");
    await store.loadMore();
    expect(store.results).toHaveLength(50);
    expect(store.error).toBe("未找到 scoop 根目录");
  });
});
