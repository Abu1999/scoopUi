import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

type Handler = (e: { payload: unknown }) => void;
const registered: Record<string, Handler[]> = {};
const failCalls = new Set<number>();
let listenCalls = 0;
let unlistenCalls = 0;
let failListen = false;
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: Handler) => {
    listenCalls += 1;
    if (failListen || failCalls.has(listenCalls)) return Promise.reject(new Error("listen 失败"));
    (registered[name] ??= []).push(handler);
    return Promise.resolve(() => {
      unlistenCalls += 1;
      const handlers = registered[name];
      const index = handlers ? handlers.indexOf(handler) : -1;
      if (index >= 0) handlers.splice(index, 1);
    });
  }),
}));

import { useLogsStore } from "../stores/logs";

describe("logs store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    failListen = false;
    failCalls.clear();
    listenCalls = 0;
    unlistenCalls = 0;
    for (const k of Object.keys(registered)) delete registered[k];
  });

  it("init 注册两个事件监听", async () => {
    const store = useLogsStore();
    await store.init();
    expect(Object.keys(registered).sort()).toEqual(["scoop-log", "scoop-task-end"]);
  });

  it("init 失败后可重试注册监听", async () => {
    failListen = true;
    const store = useLogsStore();
    await expect(store.init()).rejects.toThrow("listen 失败");
    expect(store.initialized).toBe(false);
    failListen = false;
    await store.init();
    expect(store.initialized).toBe(true);
    expect(Object.keys(registered).sort()).toEqual(["scoop-log", "scoop-task-end"]);
  });

  it("部分注册失败后重试不会重复注册监听", async () => {
    failCalls.add(2);
    const store = useLogsStore();
    await expect(store.init()).rejects.toBeTruthy();
    expect(store.initialized).toBe(false);
    expect(unlistenCalls).toBe(1);
    await store.init();
    expect(store.initialized).toBe(true);
    expect(registered["scoop-log"]).toHaveLength(1);
    expect(registered["scoop-task-end"]).toHaveLength(1);
    expect(listenCalls).toBe(4);
  });

  it("并发 init 只注册一次监听", async () => {
    const store = useLogsStore();
    await Promise.all([store.init(), store.init()]);
    expect(listenCalls).toBe(2);
    expect(registered["scoop-log"]).toHaveLength(1);
    expect(registered["scoop-task-end"]).toHaveLength(1);
  });

  it("收到 scoop-log 追加行", async () => {
    const store = useLogsStore();
    await store.init();
    registered["scoop-log"][0]({ payload: { taskId: "t1", stream: "stdout", line: "hello" } });
    expect(store.lines[store.lines.length - 1]).toMatchObject({ taskId: "t1", stream: "stdout", line: "hello" });
  });

  it("start/end 维护 running 与 history", async () => {
    const store = useLogsStore();
    await store.init();
    store.start("t1", "scoop --version");
    expect(store.running["t1"]).toBe(true);
    registered["scoop-task-end"][0]({ payload: { taskId: "t1", code: 0, success: true } });
    expect(store.running["t1"]).toBe(false);
    expect(store.history[0]).toMatchObject({ taskId: "t1", label: "scoop --version", success: true });
  });
});
