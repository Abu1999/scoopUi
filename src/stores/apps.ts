import { defineStore } from "pinia";
import { listInstalledApps, listUpdates } from "../api/commands";
import type { InstalledApp, UpdateInfo } from "../api/types";
import { appKey } from "../utils/appkey";

export const useAppsStore = defineStore("apps", {
  state: () => ({
    apps: [] as InstalledApp[],
    updates: {} as Record<string, UpdateInfo>,
    selectedKey: null as string | null,
    loading: false,
    error: null as string | null,
    seq: 0,
  }),
  getters: {
    selected(state): InstalledApp | null {
      return state.apps.find((a) => appKey(a) === state.selectedKey) ?? null;
    },
    updateCount(state): number {
      return Object.keys(state.updates).length;
    },
    updateFor(state) {
      return (app: InstalledApp): UpdateInfo | null => state.updates[appKey(app)] ?? null;
    },
  },
  actions: {
    async load() {
      const seq = ++this.seq;
      this.loading = true;
      this.error = null;
      try {
        const apps = await listInstalledApps();
        if (seq !== this.seq) return;
        this.apps = apps;
        try {
          const list = await listUpdates();
          if (seq !== this.seq) return;
          this.updates = Object.fromEntries(list.map((u) => [appKey(u), u]));
        } catch {
          if (seq === this.seq) this.updates = {};
        }
        if (!this.apps.some((a) => appKey(a) === this.selectedKey)) {
          this.selectedKey = this.apps[0] ? appKey(this.apps[0]) : null;
        }
      } catch (e) {
        if (seq !== this.seq) return;
        this.apps = [];
        this.error = String(e);
      } finally {
        if (seq === this.seq) this.loading = false;
      }
    },
    select(key: string) {
      this.selectedKey = key;
    },
  },
});
