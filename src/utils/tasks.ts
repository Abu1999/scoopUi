import { watch } from "vue";
import { useLogsStore } from "../stores/logs";
import type { TaskRecord } from "../stores/logs";
import { errorHint } from "./errorHint";

const TASK_TIMEOUT_MS = 30 * 60 * 1000;
let counter = 0;

export function newTaskId(): string {
  counter += 1;
  return `t-${Date.now()}-${counter}`;
}

export async function runTask(
  label: string,
  start: (taskId: string) => Promise<void>,
): Promise<TaskRecord> {
  const logs = useLogsStore();
  const taskId = newTaskId();
  logs.start(taskId, label);
  let timer: ReturnType<typeof setTimeout> | null = null;
  const done = new Promise<TaskRecord>((resolve) => {
    const stop = watch(
      () => logs.running[taskId],
      (running) => {
        if (running === false) {
          if (timer) clearTimeout(timer);
          stop();
          const record =
            logs.history.find((r) => r.taskId === taskId) ?? {
              taskId,
              label,
              success: false,
              code: null,
              at: Date.now(),
            };
          const hint = record.success
            ? null
            : errorHint(
                logs.lines
                  .filter((l) => l.taskId === taskId && l.stream === "stderr")
                  .map((l) => l.line),
              );
          resolve({ ...record, hint });
        }
      },
    );
    timer = setTimeout(() => {
      logs.fail(taskId, "等待任务结束事件超时");
    }, TASK_TIMEOUT_MS);
  });
  try {
    await logs.init();
  } catch {
    logs.fail(taskId, "日志监听未就绪，任务未执行");
    return done;
  }
  try {
    await start(taskId);
  } catch (e) {
    logs.fail(taskId, String(e));
  }
  return done;
}
