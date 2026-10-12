import { defineStore } from "pinia";
import { installPackage, matchRegistryApps, runRegistryUninstall } from "../api/commands";
import type { MigrationEntry, MigrationMatch } from "../api/types";
import { registerInstall } from "../utils/postinstall";
import { runTask } from "../utils/tasks";
import { useAppsStore } from "./apps";
import { useUiStore } from "./ui";

export interface InstallResult {
  ok: boolean;
  warning: string | null;
  error: string | null;
}

export const useMigrateStore = defineStore("migrate", {
  state: () => ({
    entries: [] as MigrationEntry[],
    scanned: false,
    loading: false,
    error: null as string | null,
    showManaged: false,
  }),
  getters: {
    visible(state): MigrationEntry[] {
      return state.showManaged ? state.entries : state.entries.filter((e) => !e.app.managed);
    },
    managedCount(state): number {
      return state.entries.filter((e) => e.app.managed).length;
    },
    matchedCount(state): number {
      return state.entries.filter((e) => !e.app.managed && e.matches.length > 0).length;
    },
  },
  actions: {
    async scan() {
      this.loading = true;
      this.error = null;
      try {
        this.entries = await matchRegistryApps();
        this.scanned = true;
      } catch (e) {
        this.entries = [];
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },
    async install(
      entry: MigrationEntry,
      match: MigrationMatch,
      uninstallFirst: boolean,
    ): Promise<InstallResult> {
      useUiStore().logDrawerOpen = true;
      let warning: string | null = null;
      if (uninstallFirst) {
        const uninstall = await runTask(`卸载旧版 ${entry.app.name}`, (taskId) =>
          runRegistryUninstall(entry.app.key_path, taskId),
        );
        if (!uninstall.success) {
          warning = `旧版卸载失败（退出码 ${uninstall.code ?? "未知"}）${
            uninstall.hint ? "：" + uninstall.hint : ""
          }，已继续安装`;
        }
      }
      const record = await runTask(`安装 ${match.name}`, (taskId) =>
        installPackage(match.name, match.bucket, null, false, taskId),
      );
      if (record.success) {
        await registerInstall(match.name, false);
        await useAppsStore().load();
        return { ok: true, warning, error: null };
      }
      return {
        ok: false,
        warning,
        error: record.hint
          ? `步骤失败（退出码 ${record.code ?? "未知"}）：${record.hint}`
          : `步骤失败（退出码 ${record.code ?? "未知"}）`,
      };
    },
  },
});
