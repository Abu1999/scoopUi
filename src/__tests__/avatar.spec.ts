import { describe, expect, it } from "vitest";
import { avatarColor, avatarLetter } from "../utils/avatar";

describe("avatarLetter", () => {
  it("取首字符并大写", () => {
    expect(avatarLetter("git")).toBe("G");
    expect(avatarLetter("7zip")).toBe("7");
  });

  it("空字符串回退问号", () => {
    expect(avatarLetter("   ")).toBe("?");
  });
});

describe("avatarColor", () => {
  it("同名稳定且来自固定色板", () => {
    const c1 = avatarColor("python");
    const c2 = avatarColor("python");
    expect(c1).toBe(c2);
    expect(c1).toMatch(/^#[0-9a-f]{6}$/i);
  });

  it("不同名可给出不同颜色", () => {
    const names = ["git", "7zip", "python", "cmake", "mingw", "flutter", "dark", "android-clt"];
    expect(new Set(names.map(avatarColor)).size).toBeGreaterThan(1);
  });
});
