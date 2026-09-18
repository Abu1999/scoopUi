import { defineStore } from "pinia";
import { fetchIcon } from "../api/commands";

const MAX_CONCURRENT = 6;

export const useIconsStore = defineStore("icons", {
  state: () => ({
    map: {} as Record<string, string>,
    failed: {} as Record<string, true>,
    pending: {} as Record<string, true>,
    queue: [] as string[],
    active: 0,
  }),
  actions: {
    resolve(source: string | null | undefined): string | null {
      if (!source) return null;
      return this.map[source] ?? null;
    },
    ensure(source: string | null | undefined) {
      if (!source || this.map[source] || this.failed[source] || this.pending[source]) return;
      this.pending[source] = true;
      this.queue.push(source);
      this.pump();
    },
    pump() {
      while (this.active < MAX_CONCURRENT && this.queue.length > 0) {
        const source = this.queue.shift()!;
        this.active += 1;
        fetchIcon(source)
          .then((url) => {
            if (url) {
              this.map[source] = url;
            } else {
              this.failed[source] = true;
            }
          })
          .catch(() => {
            this.failed[source] = true;
          })
          .finally(() => {
            delete this.pending[source];
            this.active -= 1;
            this.pump();
          });
      }
    },
  },
});
