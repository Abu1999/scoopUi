import { defineStore } from "pinia";
import { listInstalledApps, listUpdates } from "../api/commands";
import type { InstalledApp, UpdateInfo } from "../api/types";

export const useAppsStore = defineStore("apps", {
  state: () => ({
    apps: [] as InstalledApp[],
    updates: {} as Record<string, UpdateInfo>,
    selectedName: null as string | null,
    loading: false,
    error: null as string | null,
    seq: 0,
  }),
  getters: {
    selected(state): InstalledApp | null {
      return state.apps.find((a) => a.name === state.selectedName) ?? null;
    },
    updateCount(state): number {
      return Object.keys(state.updates).length;
    },
    updateFor(state) {
      return (name: string): UpdateInfo | null => state.updates[name] ?? null;
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
          this.updates = Object.fromEntries(list.map((u) => [u.name, u]));
        } catch {
          if (seq === this.seq) this.updates = {};
        }
        if (!this.apps.some((a) => a.name === this.selectedName)) {
          this.selectedName = this.apps[0]?.name ?? null;
        }
      } catch (e) {
        if (seq !== this.seq) return;
        this.apps = [];
        this.error = String(e);
      } finally {
        if (seq === this.seq) this.loading = false;
      }
    },
    select(name: string) {
      this.selectedName = name;
    },
  },
});
