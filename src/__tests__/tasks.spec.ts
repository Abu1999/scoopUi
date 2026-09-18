import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useLogsStore } from "../stores/logs";
import { newTaskId, runTask } from "../utils/tasks";

const listenMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

describe("runTask", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    listenMock.mockReset();
    listenMock.mockResolvedValue(() => {});
  });

  it("登记任务、等待结束事件并返回记录", async () => {
    const logs = useLogsStore();
    let id = "";
    const pending = runTask("安装 git", async (taskId) => {
      id = taskId;
    });
    await flushPromises();
    expect(id).toMatch(/^t-/);
    expect(logs.running[id]).toBe(true);
    expect(logs.labels[id]).toBe("安装 git");
    logs.end({ taskId: id, code: 0, success: true });
    const record = await pending;
    expect(record.success).toBe(true);
    expect(record.taskId).toBe(id);
    expect(logs.busy).toBe(false);
  });

  it("启动被拒时记录失败并返回失败记录", async () => {
    const logs = useLogsStore();
    const record = await runTask("安装 git", async () => {
      throw "已有任务正在运行，请等待完成或取消后再试";
    });
    expect(record.success).toBe(false);
    expect(record.code).toBeNull();
    expect(logs.running[record.taskId]).toBe(false);
    expect(logs.history[0].success).toBe(false);
    expect(logs.lines.some((l) => l.line.includes("已有任务正在运行"))).toBe(true);
  });

  it("监听未就绪时不启动任务并落地失败", async () => {
    listenMock.mockRejectedValueOnce(new Error("listen 失败"));
    const logs = useLogsStore();
    let started = false;
    const record = await runTask("安装 git", async () => {
      started = true;
    });
    expect(started).toBe(false);
    expect(record.success).toBe(false);
    expect(logs.lines.some((l) => l.line.includes("日志监听未就绪"))).toBe(true);
  });

  it("长时间未收到结束事件时超时落地失败", async () => {
    vi.useFakeTimers();
    try {
      const logs = useLogsStore();
      const pending = runTask("安装 git", async () => {});
      await vi.advanceTimersByTimeAsync(30 * 60 * 1000);
      const record = await pending;
      expect(record.success).toBe(false);
      expect(logs.lines.some((l) => l.line.includes("超时"))).toBe(true);
    } finally {
      vi.useRealTimers();
    }
  });

  it("newTaskId 递增且带 t- 前缀", () => {
    expect(newTaskId()).toMatch(/^t-/);
    expect(newTaskId()).not.toBe(newTaskId());
  });
});
