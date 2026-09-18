import { watch } from "vue";
import { useLogsStore } from "../stores/logs";
import type { TaskRecord } from "../stores/logs";

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
  const done = new Promise<TaskRecord>((resolve) => {
    const stop = watch(
      () => logs.running[taskId],
      (running) => {
        if (running === false) {
          stop();
          resolve(
            logs.history.find((r) => r.taskId === taskId) ?? {
              taskId,
              label,
              success: false,
              code: null,
              at: Date.now(),
            },
          );
        }
      },
    );
  });
  try {
    await start(taskId);
  } catch (e) {
    logs.fail(taskId, String(e));
  }
  return done;
}
