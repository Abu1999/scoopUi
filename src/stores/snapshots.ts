import { defineStore } from "pinia";
import {
  bucketAdd,
  createSnapshot,
  deleteSnapshot,
  installPackage,
  listSnapshots,
  restoreSnapshotBuckets,
  restoreSnapshotConfig,
  restoreSnapshotPlan,
} from "../api/commands";
import type { RestoreBucketsResult, RestorePlan, SnapshotApp, SnapshotSummary } from "../api/types";
import { runTask } from "../utils/tasks";
import { useAppsStore } from "./apps";
import { useBucketsStore } from "./buckets";
import { useUiStore } from "./ui";

export type InstallStepStatus = "pending" | "running" | "success" | "failed" | "skipped";

export interface InstallStep {
  key: string;
  label: string;
  status: InstallStepStatus;
}

export const useSnapshotsStore = defineStore("snapshots", {
  state: () => ({
    list: [] as SnapshotSummary[],
    selectedId: null as string | null,
    plan: null as RestorePlan | null,
    loading: false,
    error: null as string | null,
    restoring: false,
    steps: [] as InstallStep[],
    restoreError: null as string | null,
  }),
  getters: {
    selected(state): SnapshotSummary | null {
      return state.list.find((s) => s.id === state.selectedId) ?? null;
    },
  },
  actions: {
    async load() {
      this.loading = true;
      this.error = null;
      try {
        this.list = await listSnapshots();
        if (!this.list.some((s) => s.id === this.selectedId)) {
          this.selectedId = this.list[0]?.id ?? null;
        }
      } catch (e) {
        this.list = [];
        this.error = String(e);
      }
      this.loading = false;
      if (!this.selectedId) {
        this.plan = null;
        return;
      }
      try {
        await this.loadPlan();
      } catch (e) {
        this.error = String(e);
      }
    },
    async loadPlan() {
      const id = this.selectedId;
      if (!id) return;
      const plan = await restoreSnapshotPlan(id);
      if (this.selectedId !== id) return;
      this.plan = plan;
    },
    async select(id: string) {
      this.selectedId = id;
      this.error = null;
      this.restoreError = null;
      this.steps = [];
      try {
        await this.loadPlan();
      } catch (e) {
        this.error = String(e);
      }
    },
    async refreshPlan() {
      if (!this.selectedId) return;
      this.plan = await restoreSnapshotPlan(this.selectedId);
    },
    async create() {
      const s = await createSnapshot();
      this.selectedId = s.id;
      await this.load();
    },
    async remove() {
      if (!this.selectedId) return;
      await deleteSnapshot(this.selectedId);
      this.selectedId = null;
      await this.load();
    },
    async restoreConfigAndBuckets(): Promise<boolean> {
      const id = this.selectedId;
      if (!id) return false;
      this.restoreError = null;
      this.steps = [];
      this.restoring = true;
      useUiStore().logDrawerOpen = true;
      try {
        await restoreSnapshotConfig(id);
      } catch (e) {
        this.restoreError = String(e);
        this.restoring = false;
        return false;
      }
      let result: RestoreBucketsResult;
      try {
        result = await restoreSnapshotBuckets(id);
      } catch (e) {
        this.restoreError = String(e);
        this.restoring = false;
        return false;
      }
      this.steps = result.missing
        .filter((b) => b.source)
        .map((b) => ({ key: `bucket:${b.name}`, label: `添加软件源 ${b.name}`, status: "pending" as const }));
      let failed = false;
      for (const step of this.steps) {
        if (failed) {
          step.status = "skipped";
          continue;
        }
        step.status = "running";
        const bucket = result.missing.find((b) => step.key === `bucket:${b.name}`)!;
        const record = await runTask(step.label, (taskId) => bucketAdd(bucket.name, bucket.source, taskId));
        if (record.success) {
          step.status = "success";
        } else {
          step.status = "failed";
          this.restoreError = `步骤失败（退出码 ${record.code ?? "未知"}）`;
          failed = true;
        }
      }
      this.restoring = false;
      if (!failed) {
        await Promise.all([useAppsStore().load(), useBucketsStore().load(), this.refreshPlan()]);
      }
      return !failed;
    },
    async installApp(app: SnapshotApp): Promise<boolean> {
      if (!this.selectedId) return false;
      const record = await runTask(
        `安装 ${app.name}${app.global ? "（全局）" : ""}`,
        (taskId) => installPackage(app.name, app.bucket, null, app.global, taskId),
      );
      if (record.success) {
        await Promise.all([useAppsStore().load(), this.refreshPlan()]);
        return true;
      }
      this.restoreError = `步骤失败（退出码 ${record.code ?? "未知"}）`;
      return false;
    },
    async installAllMissing(): Promise<boolean> {
      const plan = this.plan;
      if (!plan || plan.missing_apps.length === 0) return false;
      this.restoreError = null;
      this.restoring = true;
      this.steps = plan.missing_apps.map((a) => ({
        key: `install:${a.name}@${a.global}`,
        label: `安装 ${a.name}${a.global ? "（全局）" : ""}`,
        status: "pending" as const,
      }));
      useUiStore().logDrawerOpen = true;
      let failed = false;
      for (const step of this.steps) {
        if (failed) {
          step.status = "skipped";
          continue;
        }
        step.status = "running";
        const app = plan.missing_apps.find((a) => step.key === `install:${a.name}@${a.global}`)!;
        const record = await runTask(step.label, (taskId) =>
          installPackage(app.name, app.bucket, null, app.global, taskId),
        );
        if (record.success) {
          step.status = "success";
        } else {
          step.status = "failed";
          this.restoreError = `步骤失败（退出码 ${record.code ?? "未知"}）`;
          failed = true;
        }
      }
      this.restoring = false;
      if (!failed) {
        await Promise.all([useAppsStore().load(), this.refreshPlan()]);
      }
      return !failed;
    },
  },
});
