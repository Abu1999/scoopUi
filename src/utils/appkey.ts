export function appKey(app: { name: string; global: boolean }): string {
  return app.global ? `${app.name}@global` : app.name;
}
