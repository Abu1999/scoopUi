import { registerAppInstall } from "../api/commands";

export async function registerInstall(name: string, global: boolean): Promise<string[]> {
  try {
    const list = await registerAppInstall(name, global);
    return Array.isArray(list) ? list : [];
  } catch {
    return [];
  }
}
