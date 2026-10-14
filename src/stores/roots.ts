import { defineStore } from "pinia";
import { adoptScoopDir, getScoopEnv, moveScoopDir, removeScoopEnv } from "../api/commands";
import type { ScoopEnvInfo } from "../api/types";

export const useRootsStore = defineStore("roots", {
  state: () => ({
    env: null as ScoopEnvInfo | null,
    loading: false,
    error: "",
  }),
  actions: {
    async load() {
      this.loading = true;
      try {
        this.env = await getScoopEnv();
        this.error = "";
      } catch (e) {
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },
    async adopt(path: string) {
      this.env = await adoptScoopDir(path);
    },
    async move(path: string) {
      this.env = await moveScoopDir(path);
    },
    async remove() {
      await removeScoopEnv();
      this.env = null;
    },
  },
});
