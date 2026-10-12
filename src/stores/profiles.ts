import { defineStore } from "pinia";
import {
  applyProfileSettings,
  bucketAdd,
  deleteProfile,
  exportProfile,
  getProfile,
  importProfile,
  installPackage,
  listProfiles,
  profileDiff,
  renameProfile,
  saveProfileFromCurrent,
  uninstallPackage,
} from "../api/commands";
import type { Profile, ProfileDiff, ProfileSummary } from "../api/types";
import { runTask } from "../utils/tasks";
import { useAppsStore } from "./apps";
import { useBucketsStore } from "./buckets";
import { useUiStore } from "./ui";

export type ApplyStepStatus = "pending" | "running" | "success" | "failed" | "skipped";

export interface ApplyStep {
  key: string;
  label: string;
  status: ApplyStepStatus;
}

function runTaskFor(step: ApplyStep, diff: ProfileDiff) {
  if (step.key.startsWith("bucket:")) {
    const bucket = diff.missing_buckets.find((b) => step.key === `bucket:${b.name}`)!;
    return runTask(step.label, (taskId) => bucketAdd(bucket.name, bucket.source, taskId));
  }
  if (step.key.startsWith("install:")) {
    const app = diff.missing_apps.find((a) => step.key === `install:${a.name}@${a.global}`)!;
    return runTask(step.label, (taskId) =>
      installPackage(app.name, app.bucket, null, app.global, taskId),
    );
  }
  const app = diff.extra_apps.find((a) => step.key === `uninstall:${a.name}@${a.global}`)!;
  return runTask(step.label, (taskId) => uninstallPackage(app.name, app.global, false, taskId));
}

export const useProfilesStore = defineStore("profiles", {
  state: () => ({
    list: [] as ProfileSummary[],
    selectedName: null as string | null,
    detail: null as Profile | null,
    diff: null as ProfileDiff | null,
    loading: false,
    error: null as string | null,
    applying: false,
    steps: [] as ApplyStep[],
    applyError: null as string | null,
  }),
  getters: {
    hasDiff(state): boolean {
      const d = state.diff;
      if (!d) return false;
      return (
        d.missing_buckets.length > 0 ||
        d.missing_apps.length > 0 ||
        d.extra_apps.length > 0 ||
        d.settings.length > 0
      );
    },
  },
  actions: {
    async load() {
      this.loading = true;
      this.error = null;
      try {
        this.list = await listProfiles();
        if (!this.list.some((p) => p.name === this.selectedName)) {
          this.selectedName = this.list[0]?.name ?? null;
        }
      } catch (e) {
        this.list = [];
        this.error = String(e);
      }
      this.loading = false;
      if (!this.selectedName) {
        this.detail = null;
        this.diff = null;
        return;
      }
      try {
        await this.loadDetail();
      } catch (e) {
        this.error = String(e);
      }
    },
    async loadDetail() {
      const name = this.selectedName;
      if (!name) return;
      const [detailResult, diffResult] = await Promise.allSettled([getProfile(name), profileDiff(name)]);
      if (this.selectedName !== name) return;
      this.detail = detailResult.status === "fulfilled" ? detailResult.value : null;
      this.diff = diffResult.status === "fulfilled" ? diffResult.value : null;
      const failure =
        detailResult.status === "rejected"
          ? detailResult.reason
          : diffResult.status === "rejected"
            ? diffResult.reason
            : null;
      if (failure !== null && failure !== undefined) {
        this.error = String(failure);
      }
    },
    async select(name: string) {
      this.selectedName = name;
      this.error = null;
      try {
        await this.loadDetail();
      } catch (e) {
        this.error = String(e);
      }
    },
    async refreshDiff() {
      const name = this.selectedName;
      if (!name) return;
      this.diff = await profileDiff(name);
    },
    async saveFromCurrent(name: string) {
      await saveProfileFromCurrent(name);
      this.selectedName = name;
      await this.load();
    },
    async importFrom(path: string, name: string) {
      await importProfile(path, name);
      this.selectedName = name;
      await this.load();
    },
    async exportTo(path: string) {
      if (!this.selectedName) return;
      await exportProfile(this.selectedName, path);
    },
    async rename(newName: string) {
      if (!this.selectedName) return;
      await renameProfile(this.selectedName, newName);
      this.selectedName = newName;
      await this.load();
    },
    async remove() {
      if (!this.selectedName) return;
      await deleteProfile(this.selectedName);
      this.selectedName = null;
      await this.load();
    },
    async apply(uninstallExtra: boolean): Promise<boolean> {
      const name = this.selectedName;
      const profile = this.detail;
      if (!name || !profile) return false;
      const diff = await profileDiff(name);
      if (this.selectedName !== name) return false;
      this.diff = diff;
      const steps: ApplyStep[] = [];
      if (diff.settings.length > 0) {
        steps.push({ key: "settings", label: "写入下载加速设置", status: "pending" });
      }
      for (const b of diff.missing_buckets) {
        steps.push({ key: `bucket:${b.name}`, label: `添加软件源 ${b.name}`, status: "pending" });
      }
      for (const a of diff.missing_apps) {
        steps.push({
          key: `install:${a.name}@${a.global}`,
          label: `安装 ${a.name}${a.global ? "（全局）" : ""}`,
          status: "pending",
        });
      }
      if (uninstallExtra) {
        for (const a of diff.extra_apps) {
          steps.push({
            key: `uninstall:${a.name}@${a.global}`,
            label: `卸载 ${a.name}${a.global ? "（全局）" : ""}`,
            status: "pending",
          });
        }
      }
      this.steps = steps;
      this.applyError = null;
      this.applying = true;
      useUiStore().logDrawerOpen = true;
      let failed = false;
      for (const step of this.steps) {
        if (failed) {
          step.status = "skipped";
          continue;
        }
        step.status = "running";
        if (step.key === "settings") {
          try {
            await applyProfileSettings(profile.settings);
            step.status = "success";
          } catch (e) {
            step.status = "failed";
            this.applyError = String(e);
            failed = true;
          }
          continue;
        }
        const record = await runTaskFor(step, diff);
        if (record.success) {
          step.status = "success";
        } else {
          step.status = "failed";
          this.applyError = record.hint
            ? `步骤失败（退出码 ${record.code ?? "未知"}）：${record.hint}`
            : `步骤失败（退出码 ${record.code ?? "未知"}）`;
          failed = true;
        }
      }
      this.applying = false;
      if (!failed) {
        await Promise.all([useAppsStore().load(), useBucketsStore().load()]);
      }
      return !failed;
    },
  },
});
