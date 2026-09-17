import { defineStore } from "pinia";
import { addRoot, detectRoots, getActiveRoot, setActiveRoot } from "../api/commands";
import type { RootInfo } from "../api/types";

export const useRootsStore = defineStore("roots", {
  state: () => ({
    roots: [] as RootInfo[],
    active: null as RootInfo | null,
    loading: false,
    error: null as string | null,
  }),
  actions: {
    async load() {
      this.loading = true;
      this.error = null;
      try {
        this.active = await getActiveRoot();
        this.roots = await detectRoots();
      } catch (e) {
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },
    async setActive(path: string) {
      this.error = null;
      try {
        this.active = await setActiveRoot(path);
        await this.load();
      } catch (e) {
        this.error = String(e);
      }
    },
    async add(path: string) {
      this.error = null;
      try {
        await addRoot(path);
        await this.load();
      } catch (e) {
        this.error = String(e);
      }
    },
  },
});
