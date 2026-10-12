const RULES: Array<{ keys: string[]; hint: string }> = [
  {
    keys: [
      "timed out",
      "timeout",
      "could not resolve",
      "connection",
      "econnreset",
      "无法连接",
      "临时故障",
      "下载失败",
    ],
    hint: "检查网络或切换下载加速代理后重试",
  },
  {
    keys: ["hash check failed", "hash mismatch", "哈希校验失败", "哈希不匹配"],
    hint: "镜像未同步：运行 scoop update 或清理缓存后重试",
  },
  {
    keys: ["access is denied", "拒绝访问", "requires admin", "admin rights", "需要管理员"],
    hint: "权限不足：以管理员身份运行或改用全局安装",
  },
  {
    keys: ["already installed", "already exists", "conflict", "冲突"],
    hint: "先卸载同名应用再试",
  },
];

export function errorHint(lines: string[]): string | null {
  if (lines.length === 0) return null;
  const text = lines.join("\n").toLowerCase();
  for (const rule of RULES) {
    if (rule.keys.some((k) => text.includes(k))) return rule.hint;
  }
  return null;
}
