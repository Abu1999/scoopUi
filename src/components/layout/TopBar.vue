<template>
  <div class="topbar">
    <div class="left">
      <span class="label">根目录</span>
      <span class="path" data-testid="topbar-root-path">
        {{ roots.env?.path ?? "未找到 scoop 安装目录" }}
      </span>
      <span v-if="roots.env" class="count">{{ roots.env.app_count }} 应用</span>
      <n-tag v-if="roots.env && !roots.env.valid" type="error" size="small">无效</n-tag>
    </div>
    <div class="right">
      <n-button size="small" :loading="anyLoading" @click="refresh">刷新</n-button>
      <n-button size="small" :type="ui.logDrawerOpen ? 'primary' : 'default'" @click="ui.toggleLogs()">
        日志
      </n-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { NButton, NTag, useMessage } from "naive-ui";
import { useRootsStore } from "../../stores/roots";
import { useAppsStore } from "../../stores/apps";
import { useBucketsStore } from "../../stores/buckets";
import { useUiStore } from "../../stores/ui";

const roots = useRootsStore();
const apps = useAppsStore();
const buckets = useBucketsStore();
const ui = useUiStore();
const message = useMessage();
const refreshing = ref(false);

const anyLoading = computed(
  () => refreshing.value || roots.loading || apps.loading || buckets.loading,
);

async function refresh() {
  if (anyLoading.value) return;
  refreshing.value = true;
  const started = Date.now();
  try {
    await Promise.all([roots.load(), apps.load(), buckets.load()]);
    if (roots.error) message.error(roots.error);
  } finally {
    const elapsed = Date.now() - started;
    if (elapsed < 500) {
      await new Promise((resolve) => setTimeout(resolve, 500 - elapsed));
    }
    refreshing.value = false;
  }
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
.path {
  font-size: 13px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 480px;
}
.count {
  color: var(--text-dim);
  font-size: 12px;
  white-space: nowrap;
}
</style>
