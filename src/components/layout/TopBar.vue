<template>
  <div class="topbar">
    <div class="left">
      <span class="label">根目录</span>
      <n-select
        data-testid="topbar-root-select"
        style="flex: 1 1 220px; min-width: 0; max-width: 340px"
        size="small"
        :value="rootValue"
        :options="rootOptions"
        placeholder="未找到 scoop 根目录"
        @update:value="onSelectRoot"
      />
      <n-tag v-if="roots.active && !roots.active.valid" type="error" size="small">无效</n-tag>
      <n-tag v-if="roots.active?.has_portable_config" size="small" type="warning">便携配置</n-tag>
    </div>
    <div class="right">
      <n-button size="small" :loading="anyLoading" @click="refresh">刷新</n-button>
      <n-button size="small" @click="pickRoot">添加根目录</n-button>
      <n-button size="small" :type="ui.logDrawerOpen ? 'primary' : 'default'" @click="ui.toggleLogs()">
        日志
      </n-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { NButton, NSelect, NTag, useMessage } from "naive-ui";
import { open } from "@tauri-apps/plugin-dialog";
import { useRootsStore } from "../../stores/roots";
import { useAppsStore } from "../../stores/apps";
import { useBucketsStore } from "../../stores/buckets";
import { useUiStore } from "../../stores/ui";

const roots = useRootsStore();
const apps = useAppsStore();
const buckets = useBucketsStore();
const ui = useUiStore();
const message = useMessage();

const rootOptions = computed(() =>
  roots.roots.map((r) => ({
    label: `${r.path}（${r.app_count} 应用）${r.is_active ? " · 当前" : ""}`,
    value: r.path,
    is_active: r.is_active,
  })),
);
const rootValue = computed(
  () => rootOptions.value.find((o) => o.is_active)?.value ?? roots.active?.path ?? null,
);
const anyLoading = computed(() => roots.loading || apps.loading || buckets.loading);

async function onSelectRoot(path: string) {
  await roots.setActive(path);
  if (roots.error) {
    message.error(roots.error);
    return;
  }
  await Promise.all([apps.load(), buckets.load()]);
}

async function refresh() {
  await Promise.all([roots.load(), apps.load(), buckets.load()]);
  if (roots.error) message.error(roots.error);
}

async function pickRoot() {
  const dir = await open({ directory: true, title: "选择 scoop 根目录" });
  if (typeof dir !== "string") return;
  await roots.add(dir);
  if (roots.error) message.error(roots.error);
  else message.success("已添加根目录");
}
</script>

<style scoped>
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
  gap: 12px;
  flex-wrap: wrap;
  row-gap: 8px;
}
.left,
.right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.left {
  flex: 1 1 auto;
  min-width: 0;
}
.right {
  flex: 0 0 auto;
  justify-content: flex-end;
}
.label {
  color: var(--text-dim);
  font-size: 13px;
}
</style>
