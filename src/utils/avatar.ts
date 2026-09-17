const PALETTE = [
  "#2080f0",
  "#18a058",
  "#f0a020",
  "#d03050",
  "#8a2be2",
  "#00b4d8",
  "#e8590c",
  "#2b8a3e",
];

export function avatarColor(name: string): string {
  let h = 0;
  for (const ch of name) {
    h = (h * 31 + (ch.codePointAt(0) ?? 0)) >>> 0;
  }
  return PALETTE[h % PALETTE.length];
}

export function avatarLetter(name: string): string {
  const ch = [...name.trim()][0] ?? "?";
  return ch.toUpperCase();
}
