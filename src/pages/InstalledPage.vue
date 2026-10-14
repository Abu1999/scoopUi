<template>
  <div class="page">
    <div class="side">
      <div class="toolbar">
        <n-popconfirm
          :positive-button-props="{ disabled: busy || apps.updateCount === 0 }"
          @positive-click="updateAll"
        >
          <template #trigger>
            <n-button size="small" :disabled="busy || apps.updateCount === 0">
              全部更新（{{ apps.updateCount }}）
            </n-button>
          </template>
          确定更新全部 {{ apps.updateCount }} 个可更新应用？
        </n-popconfirm>
      </div>
      <n-alert v-if="apps.error" type="error" size="small" class="alert">{{ apps.error }}</n-alert>
      <AppList
        :apps="apps.apps"
        :selected="apps.selectedKey"
        :updates="apps.updates"
        class="list"
        @select="apps.select"
      />
    </div>
    <AppDetail :app="apps.selected" class="detail" />
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { NAlert, NButton, NPopconfirm, useMessage } from "naive-ui";
import AppList from "../components/app/AppList.vue";
import AppDetail from "../components/app/AppDetail.vue";
import { updatePackage } from "../api/commands";
import { useAppsStore } from "../stores/apps";
import { useLogsStore } from "../stores/logs";
import type { TaskRecord } from "../stores/logs";
import { useUiStore } from "../stores/ui";
import { runTask } from "../utils/tasks";

const apps = useAppsStore();
const logs = useLogsStore();
const ui = useUiStore();
const message = useMessage();
const busy = computed(() => logs.busy);

function failMessage(label: string, record: TaskRecord): string {
  return record.hint
    ? `${label}失败（退出码 ${record.code ?? "未知"}）：${record.hint}`
    : `${label}失败（退出码 ${record.code ?? "未知"}），详见任务日志`;
}

async function updateAll() {
  if (busy.value || apps.updateCount === 0) return;
  ui.logDrawerOpen = true;
  const record = await runTask("全部更新", (taskId) => updatePackage(null, true, taskId));
  if (!record.success) {
    message.error(failMessage("更新", record));
    return;
  }
  message.success("更新完成");
  await apps.load();
}
</script>

<style scoped>
.page {
  display: flex;
  height: 100%;
}
.side {
  width: 340px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.toolbar {
  display: flex;
  gap: 8px;
  padding: 8px 8px 0;
}
.alert {
  margin: 8px 8px 0;
}
.list {
  flex: 1;
  margin-top: 8px;
  min-height: 0;
}
.detail {
  flex: 1;
  min-width: 0;
}
</style>
