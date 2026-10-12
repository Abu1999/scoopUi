import { defineStore } from "pinia";
import { getAppSettings, setTheme } from "../api/commands";

export type ThemePreference = "system" | "light" | "dark";

function prefersDark(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export const useAppearanceStore = defineStore("appearance", {
  state: () => ({
    theme: "system" as ThemePreference,
    systemDark: prefersDark(),
    watching: false,
    loaded: false,
  }),
  getters: {
    effectiveTheme(state): "light" | "dark" {
      if (state.theme === "system") return state.systemDark ? "dark" : "light";
      return state.theme;
    },
  },
  actions: {
    async load() {
      try {
        const s = await getAppSettings();
        if (s.theme === "system" || s.theme === "light" || s.theme === "dark") {
          this.theme = s.theme;
        }
      } catch {
        this.theme = "system";
      }
      this.watchSystem();
      this.loaded = true;
    },
    watchSystem() {
      if (this.watching) return;
      if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
      this.watching = true;
      const media = window.matchMedia("(prefers-color-scheme: dark)");
      this.systemDark = media.matches;
      media.addEventListener("change", (e) => {
        this.systemDark = e.matches;
      });
    },
    async setThemePreference(theme: ThemePreference) {
      this.theme = theme;
      await setTheme(theme);
    },
  },
});
