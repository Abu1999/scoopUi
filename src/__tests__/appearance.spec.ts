import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getAppSettings: vi.fn(),
  setTheme: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);

import { useAppearanceStore } from "../stores/appearance";

describe("appearance store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("系统深色时 effectiveTheme 为 dark 并响应变化", async () => {
    let handler: ((e: { matches: boolean }) => void) | null = null;
    const mql = {
      matches: false,
      addEventListener: vi.fn((_, cb: (e: { matches: boolean }) => void) => {
        handler = cb;
      }),
    };
    vi.stubGlobal("matchMedia", vi.fn(() => mql));
    mocks.getAppSettings.mockResolvedValue({ theme: "system" });
    const store = useAppearanceStore();
    await store.load();
    expect(store.effectiveTheme).toBe("light");
    handler!({ matches: true });
    expect(store.effectiveTheme).toBe("dark");
  });

  it("浅色/深色偏好覆盖系统设置并持久化", async () => {
    vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: true, addEventListener: vi.fn() })));
    mocks.getAppSettings.mockResolvedValue({ theme: "light" });
    mocks.setTheme.mockResolvedValue(undefined);
    const store = useAppearanceStore();
    await store.load();
    expect(store.effectiveTheme).toBe("light");
    await store.setThemePreference("dark");
    expect(mocks.setTheme).toHaveBeenCalledWith("dark");
    expect(store.effectiveTheme).toBe("dark");
  });

  it("读取偏好失败时回退 system", async () => {
    vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn() })));
    mocks.getAppSettings.mockRejectedValue("读取失败");
    const store = useAppearanceStore();
    await store.load();
    expect(store.theme).toBe("system");
    expect(store.loaded).toBe(true);
  });

  it("持久化失败时回滚偏好", async () => {
    vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn() })));
    mocks.getAppSettings.mockResolvedValue({ theme: "system" });
    mocks.setTheme.mockRejectedValue("磁盘不可写");
    const store = useAppearanceStore();
    await store.load();
    await expect(store.setThemePreference("dark")).rejects.toBe("磁盘不可写");
    expect(store.theme).toBe("system");
  });
});
