import { defineStore } from "pinia";
import { refreshPackageIndex, searchPackages } from "../api/commands";
import type { PackageEntry } from "../api/types";

export const PAGE_SIZE = 50;

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
    limit: PAGE_SIZE,
    loadedAll: false,
  }),
  getters: {
    selected(state): PackageEntry | null {
      return state.results.find((p) => p.name === state.selectedName) ?? null;
    },
  },
  actions: {
    async fetchPage() {
      const seq = ++this.seq;
      this.loading = true;
      this.error = null;
      try {
        const results = await searchPackages(this.query, this.limit);
        if (seq !== this.seq) return;
        this.results = results;
        this.loadedAll = results.length < this.limit;
        if (!results.some((p) => p.name === this.selectedName)) {
          this.selectedName = results[0]?.name ?? null;
        }
      } catch (e) {
        if (seq !== this.seq) return;
        this.error = String(e);
      } finally {
        if (seq === this.seq) this.loading = false;
      }
    },
    async search(query: string) {
      this.query = query;
      this.limit = PAGE_SIZE;
      this.results = [];
      await this.fetchPage();
    },
    async loadMore() {
      if (this.loading || this.refreshing || this.loadedAll) return;
      this.limit += PAGE_SIZE;
      await this.fetchPage();
    },
    async refresh() {
      this.refreshing = true;
      this.error = null;
      try {
        this.indexCount = await refreshPackageIndex();
        await this.fetchPage();
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
