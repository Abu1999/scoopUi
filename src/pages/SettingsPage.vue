<template>
  <div class="page">
    <h3>运行环境</h3>
    <n-descriptions :column="1" size="small" bordered>
      <n-descriptions-item label="当前根目录">
        {{ roots.active?.path ?? "未设置" }}
      </n-descriptions-item>
      <n-descriptions-item label="应用 / 桶数量">
        {{ roots.active ? `${roots.active.app_count} / ${roots.active.bucket_count}` : "-" }}
      </n-descriptions-item>
      <n-descriptions-item label="全局目录">
        {{ roots.active?.global_dir ?? "-" }}
      </n-descriptions-item>
    </n-descriptions>
    <div class="actions">
      <n-button size="small" :disabled="!roots.active" @click="checkEnv">检查环境（scoop --version）</n-button>
    </div>

    <h3>配置文件</h3>
    <n-descriptions v-if="config" :column="1" size="small" bordered>
      <n-descriptions-item label="路径">{{ config.file_path }}</n-descriptions-item>
      <n-descriptions-item label="便携模式">{{ config.portable ? "是" : "否" }}</n-descriptions-item>
    </n-descriptions>
    <pre v-if="config" class="json">{{ configJson }}</pre>
    <div v-else class="empty">暂无配置信息</div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NButton, NDescriptions, NDescriptionsItem } from "naive-ui";
import { getScoopConfig, runScoop } from "../api/commands";
import type { ScoopConfig } from "../api/types";
import { useLogsStore } from "../stores/logs";
import { useRootsStore } from "../stores/roots";
import { useUiStore } from "../stores/ui";

const roots = useRootsStore();
const logs = useLogsStore();
const ui = useUiStore();
const config = ref<ScoopConfig | null>(null);

const configJson = computed(() => (config.value ? JSON.stringify(config.value.values, null, 2) : ""));

async function loadConfig() {
  config.value = null;
  if (!roots.active) return;
  try {
    config.value = await getScoopConfig();
  } catch {
    config.value = null;
  }
}

watch(() => roots.active?.path, loadConfig, { immediate: true });

async function checkEnv() {
  const taskId = `check-${Date.now()}`;
  ui.logDrawerOpen = true;
  logs.start(taskId, "scoop --version");
  try {
    await runScoop(["--version"], taskId);
  } catch (e) {
    logs.lines.push({ taskId, stream: "stderr", line: String(e) });
    logs.end({ taskId, code: null, success: false });
  }
}
</script>

<style scoped>
.page {
  padding: 16px;
  overflow: auto;
  height: 100%;
}
h3 {
  margin: 18px 0 8px;
  font-size: 14px;
}
h3:first-child {
  margin-top: 0;
}
.actions {
  margin-top: 10px;
}
.json {
  margin-top: 10px;
  background: #f7f7f9;
  border-radius: 4px;
  padding: 10px;
  font-size: 12px;
  overflow: auto;
  max-height: 240px;
}
.empty {
  color: #999;
}
</style>
