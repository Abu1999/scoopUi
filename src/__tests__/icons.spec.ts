import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ fetchIcon: vi.fn() }));
vi.mock("../api/commands", () => mocks);

import { useIconsStore } from "../stores/icons";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("icons store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("成功结果按 source 记忆化，重复 ensure 只抓一次", async () => {
    mocks.fetchIcon.mockResolvedValue("data:image/png;base64,AA==");
    const store = useIconsStore();
    store.ensure("https://x.test/favicon.ico");
    store.ensure("https://x.test/favicon.ico");
    expect(mocks.fetchIcon).toHaveBeenCalledTimes(1);
    await flushPromises();
    expect(store.resolve("https://x.test/favicon.ico")).toBe("data:image/png;base64,AA==");
    store.ensure("https://x.test/favicon.ico");
    expect(mocks.fetchIcon).toHaveBeenCalledTimes(1);
  });

  it("fetchIcon 返回 null 进入会话负缓存，不再重试", async () => {
    mocks.fetchIcon.mockResolvedValue(null);
    const store = useIconsStore();
    store.ensure("https://y.test/favicon.ico");
    await flushPromises();
    expect(store.resolve("https://y.test/favicon.ico")).toBeNull();
    store.ensure("https://y.test/favicon.ico");
    expect(mocks.fetchIcon).toHaveBeenCalledTimes(1);
  });

  it("抓取拒绝也进入负缓存", async () => {
    mocks.fetchIcon.mockRejectedValue(new Error("boom"));
    const store = useIconsStore();
    store.ensure("https://z.test/favicon.ico");
    await flushPromises();
    store.ensure("https://z.test/favicon.ico");
    expect(mocks.fetchIcon).toHaveBeenCalledTimes(1);
  });

  it("空 source 直接忽略", () => {
    const store = useIconsStore();
    store.ensure(null);
    store.ensure(undefined);
    store.ensure("");
    expect(mocks.fetchIcon).not.toHaveBeenCalled();
  });

  it("并发上限 6，完成后启动队列中的下一个", async () => {
    const tasks = Array.from({ length: 7 }, () => deferred<string | null>());
    let calls = 0;
    mocks.fetchIcon.mockImplementation(() => tasks[calls++].promise);
    const store = useIconsStore();
    for (let i = 0; i < 7; i++) {
      store.ensure(`https://s${i}.test/favicon.ico`);
    }
    expect(mocks.fetchIcon).toHaveBeenCalledTimes(6);
    tasks[0].resolve("data:image/png;base64,AA==");
    await flushPromises();
    expect(mocks.fetchIcon).toHaveBeenCalledTimes(7);
    tasks[1].resolve("data:image/png;base64,AA==");
    await flushPromises();
    expect(store.resolve("https://s1.test/favicon.ico")).toBe("data:image/png;base64,AA==");
  });
});
