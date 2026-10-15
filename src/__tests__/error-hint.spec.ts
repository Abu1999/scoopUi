import { describe, expect, it } from "vitest";
import { errorHint } from "../utils/errorHint";

describe("errorHint", () => {
  it("网络类 stderr 给出网络建议", () => {
    expect(errorHint(["Could not resolve host: github.com"])).toContain("网络");
  });
  it("哈希校验失败给出镜像建议", () => {
    expect(errorHint(["Hash check failed for git"])).toContain("镜像");
  });
  it("权限不足给出管理员建议", () => {
    expect(errorHint(["Access is denied"])).toContain("管理员");
  });
  it("manifest 冲突给出卸载建议", () => {
    expect(errorHint(["git is already installed. Use 'scoop update git'"])).toContain("卸载");
  });
  it("无匹配与空输入返回 null", () => {
    expect(errorHint(["some random output"])).toBeNull();
    expect(errorHint([])).toBeNull();
  });
  it("超时文案给出网络建议", () => {
    expect(errorHint(["等待任务结束事件超时"])).toContain("网络");
  });
  it("文件被占用/程序运行中给出强制删除建议", () => {
    expect(errorHint(["The process cannot access the file because it is being used by another process."])).toContain(
      "强制删除",
    );
    expect(errorHint(["The following instances of \"git\" are still running. Close them and try again."])).toContain(
      "强制删除",
    );
  });
  it("缺少 git 给出安装 git 建议", () => {
    expect(errorHint(["ERROR Git is required for buckets. Run 'scoop install git' and try again."])).toContain(
      "先在商店安装 git",
    );
  });
});
