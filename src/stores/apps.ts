import { defineStore } from "pinia";
import { listInstalledApps } from "../api/commands";
import type { InstalledApp } from "../api/types";

export const useAppsStore = defineStore("apps", {
  state: () => ({
    apps: [] as InstalledApp[],
    selectedName: null as string | null,
    loading: false,
    error: null as string | null,
  }),
  getters: {
    selected(state): InstalledApp | null {
      return state.apps.find((a) => a.name === state.selectedName) ?? null;
    },
  },
  actions: {
    async load() {
      this.loading = true;
      this.error = null;
      try {
        this.apps = await listInstalledApps();
        if (!this.apps.some((a) => a.name === this.selectedName)) {
          this.selectedName = this.apps[0]?.name ?? null;
        }
      } catch (e) {
        this.apps = [];
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },
    select(name: string) {
      this.selectedName = name;
    },
  },
});
