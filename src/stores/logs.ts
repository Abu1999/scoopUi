import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import type { ScoopLogEvent, ScoopTaskEndEvent } from "../api/types";

export interface LogLine {
  taskId: string;
  stream: "stdout" | "stderr" | "meta";
  line: string;
}

export interface TaskRecord {
  taskId: string;
  label: string;
  success: boolean;
  code: number | null;
  at: number;
}

export const useLogsStore = defineStore("logs", {
  state: () => ({
    lines: [] as LogLine[],
    running: {} as Record<string, boolean>,
    labels: {} as Record<string, string>,
    history: [] as TaskRecord[],
    initialized: false,
    initPromise: null as Promise<void> | null,
  }),
  actions: {
    async init() {
      if (this.initialized) return;
      if (!this.initPromise) {
        this.initPromise = (async () => {
          await listen<ScoopLogEvent>("scoop-log", (event) => this.append(event.payload));
          await listen<ScoopTaskEndEvent>("scoop-task-end", (event) => this.end(event.payload));
          this.initialized = true;
        })();
      }
      try {
        await this.initPromise;
      } catch (e) {
        this.initPromise = null;
        throw e;
      }
    },
    start(taskId: string, label: string) {
      this.labels[taskId] = label;
      this.running[taskId] = true;
      this.lines.push({ taskId, stream: "meta", line: `▶ ${label}` });
    },
    append(e: ScoopLogEvent) {
      this.lines.push({ taskId: e.taskId, stream: e.stream, line: e.line });
    },
    end(e: ScoopTaskEndEvent) {
      this.running[e.taskId] = false;
      const label = this.labels[e.taskId] ?? e.taskId;
      this.history.unshift({
        taskId: e.taskId,
        label,
        success: e.success,
        code: e.code,
        at: Date.now(),
      });
      this.lines.push({
        taskId: e.taskId,
        stream: "meta",
        line: e.success ? `■ ${label} 完成` : `■ ${label} 失败（退出码 ${e.code ?? "未知"}）`,
      });
    },
    clear() {
      this.lines = [];
    },
  },
});
