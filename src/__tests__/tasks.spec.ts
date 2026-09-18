import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it } from "vitest";
import { useLogsStore } from "../stores/logs";
import { newTaskId, runTask } from "../utils/tasks";

describe("runTask", () => {
  beforeEach(() => setActivePinia(createPinia()));

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

  it("newTaskId 递增且带 t- 前缀", () => {
    expect(newTaskId()).toMatch(/^t-/);
    expect(newTaskId()).not.toBe(newTaskId());
  });
});
