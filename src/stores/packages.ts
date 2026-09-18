import { defineStore } from "pinia";
import { refreshPackageIndex, searchPackages } from "../api/commands";
import type { PackageEntry } from "../api/types";

export const usePackagesStore = defineStore("packages", {
  state: () => ({
    query: "",
    results: [] as PackageEntry[],
    selectedName: null as string | null,
    loading: false,
    refreshing: false,
    indexCount: null as number | null,
    error: null as string | null,
    seq: 0,
  }),
  getters: {
    selected(state): PackageEntry | null {
      return state.results.find((p) => p.name === state.selectedName) ?? null;
    },
  },
  actions: {
    async search(query: string) {
      this.query = query;
      const seq = ++this.seq;
      this.loading = true;
      this.error = null;
      try {
        const results = await searchPackages(query);
        if (seq !== this.seq) return;
        this.results = results;
        if (!results.some((p) => p.name === this.selectedName)) {
          this.selectedName = results[0]?.name ?? null;
        }
      } catch (e) {
        if (seq !== this.seq) return;
        this.results = [];
        this.selectedName = null;
        this.error = String(e);
      } finally {
        if (seq === this.seq) this.loading = false;
      }
    },
    async refresh() {
      this.refreshing = true;
      this.error = null;
      try {
        this.indexCount = await refreshPackageIndex();
        await this.search(this.query);
      } catch (e) {
        this.error = String(e);
      } finally {
        this.refreshing = false;
      }
    },
    select(name: string) {
      this.selectedName = name;
    },
  },
});
