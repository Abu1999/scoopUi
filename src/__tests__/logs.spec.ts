import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

type Handler = (e: { payload: unknown }) => void;
const listeners: Record<string, Handler> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: Handler) => {
    listeners[name] = handler;
    return Promise.resolve(() => {});
  }),
}));

import { useLogsStore } from "../stores/logs";

describe("logs store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    for (const k of Object.keys(listeners)) delete listeners[k];
  });

  it("init 注册两个事件监听", async () => {
    const store = useLogsStore();
    await store.init();
    expect(Object.keys(listeners).sort()).toEqual(["scoop-log", "scoop-task-end"]);
  });

  it("收到 scoop-log 追加行", async () => {
    const store = useLogsStore();
    await store.init();
    listeners["scoop-log"]({ payload: { taskId: "t1", stream: "stdout", line: "hello" } });
    expect(store.lines[store.lines.length - 1]).toMatchObject({ taskId: "t1", stream: "stdout", line: "hello" });
  });

  it("start/end 维护 running 与 history", async () => {
    const store = useLogsStore();
    await store.init();
    store.start("t1", "scoop --version");
    expect(store.running["t1"]).toBe(true);
    listeners["scoop-task-end"]({ payload: { taskId: "t1", code: 0, success: true } });
    expect(store.running["t1"]).toBe(false);
    expect(store.history[0]).toMatchObject({ taskId: "t1", label: "scoop --version", success: true });
  });
});
