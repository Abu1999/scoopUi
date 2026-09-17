import { defineStore } from "pinia";

export type PageId =
  | "installed"
  | "store"
  | "buckets"
  | "maintenance"
  | "migrate"
  | "profiles"
  | "backup"
  | "settings";

export const useUiStore = defineStore("ui", {
  state: () => ({
    activePage: "installed" as PageId,
    logDrawerOpen: false,
  }),
  actions: {
    setPage(page: PageId) {
      this.activePage = page;
    },
    toggleLogs() {
      this.logDrawerOpen = !this.logDrawerOpen;
    },
  },
});
