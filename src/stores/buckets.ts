import { defineStore } from "pinia";
import { listBuckets } from "../api/commands";
import type { BucketInfo } from "../api/types";

export const useBucketsStore = defineStore("buckets", {
  state: () => ({
    buckets: [] as BucketInfo[],
    loading: false,
    error: null as string | null,
  }),
  actions: {
    async load() {
      this.loading = true;
      this.error = null;
      try {
        this.buckets = await listBuckets();
      } catch (e) {
        this.buckets = [];
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },
  },
});
