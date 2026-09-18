<template>
  <div class="page">
    <div class="toolbar">
      <n-input v-model:value="newName" size="small" placeholder="桶名称（如 extras）" class="name" />
      <n-input v-model:value="newRepo" size="small" placeholder="仓库地址（可选，已知桶可留空）" class="repo" />
      <n-button size="small" :disabled="busy || !canAdd" @click="doAdd">添加桶</n-button>
      <n-button size="small" :disabled="busy" @click="doUpdateAll">更新 Scoop 与桶</n-button>
    </div>
    <n-alert v-if="store.error" type="error" size="small" class="alert">{{ store.error }}</n-alert>
    <n-table :bordered="false" size="small">
      <thead>
        <tr>
          <th>名称</th>
          <th>来源</th>
          <th>清单数</th>
          <th>类型</th>
          <th>操作</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="b in store.buckets" :key="b.name">
          <td>{{ b.name }}</td>
          <td class="source">{{ b.source ?? "（本地目录，非 git）" }}</td>
          <td>{{ b.manifest_count }}</td>
          <td>{{ b.is_git ? "git" : "目录" }}</td>
          <td>
            <n-popconfirm @positive-click="doRemove(b.name)">
              <template #trigger>
                <n-button size="tiny" type="error" :disabled="busy">删除</n-button>
              </template>
              确定删除桶 {{ b.name }}？该桶提供的应用将无法安装/更新。
            </n-popconfirm>
          </td>
        </tr>
      </tbody>
    </n-table>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { NAlert, NButton, NInput, NPopconfirm, NTable, useMessage } from "naive-ui";
import { bucketAdd, bucketRemove, updatePackage } from "../api/commands";
import { useBucketsStore } from "../stores/buckets";
import { useLogsStore } from "../stores/logs";
import { useRootsStore } from "../stores/roots";
import { useUiStore } from "../stores/ui";
import { runTask } from "../utils/tasks";
import type { TaskRecord } from "../stores/logs";

const store = useBucketsStore();
const roots = useRootsStore();
const logs = useLogsStore();
const ui = useUiStore();
const message = useMessage();
const newName = ref("");
const newRepo = ref("");
const busy = computed(() => logs.busy);
const canAdd = computed(() => newName.value.trim().length > 0);

onMounted(reload);

async function reload() {
  await store.load();
  await roots.load();
}

async function afterTask(record: TaskRecord) {
  if (record.success) {
    await reload();
  } else {
    message.error(`操作失败（退出码 ${record.code ?? "未知"}），详见任务日志`);
  }
}

async function doAdd() {
  const name = newName.value.trim();
  if (!name) return;
  const repo = newRepo.value.trim() === "" ? null : newRepo.value.trim();
  ui.logDrawerOpen = true;
  const record = await runTask(`添加桶 ${name}`, (taskId) => bucketAdd(name, repo, taskId));
  if (record.success) {
    newName.value = "";
    newRepo.value = "";
  }
  await afterTask(record);
}

async function doRemove(name: string) {
  ui.logDrawerOpen = true;
  const record = await runTask(`删除桶 ${name}`, (taskId) => bucketRemove(name, taskId));
  await afterTask(record);
}

async function doUpdateAll() {
  ui.logDrawerOpen = true;
  const record = await runTask("更新 Scoop 与桶", (taskId) => updatePackage(null, false, false, taskId));
  await afterTask(record);
}
</script>

<style scoped>
.page {
  padding: 16px;
  overflow: auto;
  height: 100%;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}
.name {
  width: 200px;
}
.repo {
  flex: 1;
  min-width: 0;
}
.alert {
  margin-bottom: 12px;
}
.source {
  font-family: Consolas, monospace;
  font-size: 12px;
  word-break: break-all;
}
</style>
