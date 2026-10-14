import { defineStore } from "pinia";

export type PageId =
  | "installed"
  | "store"
  | "buckets"
  | "maintenance"
  | "migrate"
  | "profiles"
  | "settings";

export const useUiStore = defineStore("ui", {
  state: () => ({
    activePage: "installed" as PageId,
    logDrawerOpen: false,
    storeSearchQuery: null as string | null,
  }),
  actions: {
    setPage(page: PageId) {
      this.activePage = page;
    },
    searchInStore(query: string) {
      this.storeSearchQuery = query;
      this.activePage = "store";
    },
    toggleLogs() {
      this.logDrawerOpen = !this.logDrawerOpen;
    },
  },
});
