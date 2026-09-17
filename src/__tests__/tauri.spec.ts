import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string, args?: Record<string, unknown>) => Promise.resolve({ cmd, args })),
}));

import { invoke } from "../api/tauri";

describe("invoke 包装", () => {
  it("透传命令名与参数", async () => {
    const res = await invoke<{ cmd: string; args?: Record<string, unknown> }>("list_apps", { a: 1 });
    expect(res.cmd).toBe("list_apps");
    expect(res.args).toEqual({ a: 1 });
  });
});
