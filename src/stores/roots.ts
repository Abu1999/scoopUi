import { defineStore } from "pinia";
import {
  adoptScoopDir,
  forceRemoveScoopEnv,
  getScoopEnv,
  moveScoopDir,
  removeScoopEnv,
} from "../api/commands";
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
    /** 强制删除环境；返回仍未能删除的条目（正常为空数组）。 */
    async forceRemove(): Promise<string[]> {
      const remaining = await forceRemoveScoopEnv();
      if (remaining.length === 0) {
        this.env = null;
      }
      return remaining;
    },
  },
});
