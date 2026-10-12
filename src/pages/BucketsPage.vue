<template>
  <div class="page">
    <div class="toolbar">
      <n-select
        v-model:value="newName"
        size="small"
        filterable
        tag
        clearable
        :options="nameOptions"
        placeholder="软件源名称（可选已知软件源或直接输入）"
        class="name"
      />
      <n-input v-model:value="newRepo" size="small" placeholder="仓库地址（可选，已知软件源可留空）" class="repo" />
      <n-button size="small" :disabled="busy || !canAdd" @click="doAdd">添加软件源</n-button>
      <n-button size="small" :disabled="busy" @click="doUpdateAll">更新 Scoop 与软件源</n-button>
    </div>
    <n-alert v-if="store.error" type="error" size="small" class="alert">{{ store.error }}</n-alert>
    <div v-if="store.buckets.length === 0" class="empty">还没有软件源，添加一个开始使用</div>
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
            <n-popconfirm
              :positive-button-props="{ disabled: busy, type: 'error' }"
              @positive-click="doRemove(b.name)"
            >
              <template #trigger>
                <n-button size="tiny" type="error" :disabled="busy">删除</n-button>
              </template>
              确定删除软件源 {{ b.name }}？该软件源提供的应用将无法安装/更新。
            </n-popconfirm>
          </td>
        </tr>
      </tbody>
    </n-table>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { NAlert, NButton, NInput, NPopconfirm, NSelect, NTable, useMessage } from "naive-ui";
import { bucketAdd, bucketRemove, listKnownBuckets, updatePackage } from "../api/commands";
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
const newName = ref<string | null>(null);
const newRepo = ref("");
const known = ref<string[]>([]);
const busy = computed(() => logs.busy);
const nameOptions = computed(() => known.value.map((n) => ({ label: n, value: n })));
const canAdd = computed(() => (newName.value ?? "").trim().length > 0);

onMounted(async () => {
  try {
    known.value = await listKnownBuckets();
  } catch {
    known.value = [];
  }
  await reload();
});

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
  if (busy.value) return;
  const name = (newName.value ?? "").trim();
  if (!name) return;
  const repo = newRepo.value.trim() === "" ? null : newRepo.value.trim();
  ui.logDrawerOpen = true;
  const record = await runTask(`添加软件源 ${name}`, (taskId) => bucketAdd(name, repo, taskId));
  if (record.success) {
    newName.value = null;
    newRepo.value = "";
  }
  await afterTask(record);
}

async function doRemove(name: string) {
  if (busy.value) return;
  ui.logDrawerOpen = true;
  const record = await runTask(`删除软件源 ${name}`, (taskId) => bucketRemove(name, taskId));
  await afterTask(record);
}

async function doUpdateAll() {
  if (busy.value) return;
  ui.logDrawerOpen = true;
  const record = await runTask("更新 Scoop 与软件源", (taskId) => updatePackage(null, false, false, taskId));
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
.empty {
  padding: 24px;
  color: var(--text-dim);
  text-align: center;
}
</style>
