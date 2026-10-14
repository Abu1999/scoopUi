import { registerAppInstall } from "../api/commands";

export async function registerInstall(name: string): Promise<string[]> {
  try {
    const list = await registerAppInstall(name);
    return Array.isArray(list) ? list : [];
  } catch {
    return [];
  }
}
