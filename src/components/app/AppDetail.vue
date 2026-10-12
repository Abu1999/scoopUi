<template>
  <div class="detail">
    <div v-if="!app" class="empty">选择一个应用查看详情</div>
    <template v-else>
      <div class="head">
        <AppIcon :name="app.name" :size="64" :source="app.icon ?? app.homepage" />
        <div>
          <div class="title">
            {{ app.name }}
            <n-tag v-if="app.global" size="small">全局</n-tag>
            <n-tag v-if="app.hold" size="small" type="warning">锁定</n-tag>
          </div>
          <div class="sub">版本 {{ app.version }}<span v-if="app.bucket"> · 来自 {{ app.bucket }}</span><span v-if="app.architecture"> · {{ app.architecture }}</span></div>
          <div v-if="update" class="update-line">可更新至 {{ update.latest }}</div>
        </div>
      </div>

      <p v-if="app.description" class="desc">{{ app.description }}</p>

      <div class="actions">
        <n-button v-if="app.homepage" size="small" @click="openHomepage">打开主页</n-button>
        <n-button size="small" @click="openDir">打开安装目录</n-button>
        <n-button size="small" :disabled="busy" @click="doUpdate">更新</n-button>
        <n-popconfirm
          :positive-button-props="{ disabled: busy, type: 'error' }"
          @positive-click="doUninstall"
        >
          <template #trigger>
            <n-button size="small" type="error" :disabled="busy">卸载</n-button>
          </template>
          确定卸载 {{ app.name }}？程序文件将被删除（持久化数据保留）。
        </n-popconfirm>
        <n-button size="small" :disabled="busy" @click="toggleHold">
          {{ app.hold ? "解除锁定" : "锁定" }}
        </n-button>
      </div>

      <n-descriptions :column="1" size="small" label-placement="left" class="fields">
        <n-descriptions-item v-if="app.updated" label="更新时间">{{ app.updated }}</n-descriptions-item>
        <n-descriptions-item label="安装目录">{{ app.app_dir }}</n-descriptions-item>
      </n-descriptions>

      <div v-if="app.depends.length" class="section">
        <div class="section-title">依赖</div>
        <n-tag v-for="d in app.depends" :key="d" size="small" class="tag">{{ d }}</n-tag>
      </div>

      <div v-if="app.persist.length" class="section">
        <div class="section-title">持久化数据（persist）</div>
        <div v-for="p in app.persist" :key="p" class="persist">{{ p }}</div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { NButton, NDescriptions, NDescriptionsItem, NPopconfirm, NTag, useMessage } from "naive-ui";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import AppIcon from "./AppIcon.vue";
import type { InstalledApp } from "../../api/types";
import { setHold, uninstallPackage, updatePackage } from "../../api/commands";
import { useAppsStore } from "../../stores/apps";
import { useLogsStore } from "../../stores/logs";
import { useRootsStore } from "../../stores/roots";
import { useUiStore } from "../../stores/ui";
import { runTask } from "../../utils/tasks";
import type { TaskRecord } from "../../stores/logs";

const props = defineProps<{ app: InstalledApp | null }>();
const message = useMessage();
const apps = useAppsStore();
const logs = useLogsStore();
const roots = useRootsStore();
const ui = useUiStore();
const busy = computed(() => logs.busy);
const update = computed(() => (props.app ? apps.updateFor(props.app) : null));

async function afterTask(record: TaskRecord) {
  if (record.success) {
    await Promise.all([apps.load(), roots.load()]);
  } else {
    message.error(
      record.hint
        ? `操作失败（退出码 ${record.code ?? "未知"}）：${record.hint}`
        : `操作失败（退出码 ${record.code ?? "未知"}），详见任务日志`,
    );
  }
}

async function doUpdate() {
  if (busy.value) return;
  if (!props.app) return;
  const name = props.app.name;
  ui.logDrawerOpen = true;
  const record = await runTask(`更新 ${name}`, (taskId) => updatePackage(name, false, props.app!.global, taskId));
  await afterTask(record);
}

async function doUninstall() {
  if (busy.value) return;
  if (!props.app) return;
  const name = props.app.name;
  ui.logDrawerOpen = true;
  const record = await runTask(`卸载 ${name}`, (taskId) => uninstallPackage(name, props.app!.global, false, taskId));
  await afterTask(record);
}

async function toggleHold() {
  if (busy.value) return;
  if (!props.app) return;
  const name = props.app.name;
  const next = !props.app.hold;
  ui.logDrawerOpen = true;
  const record = await runTask(`${next ? "锁定" : "解除锁定"} ${name}`, (taskId) =>
    setHold(name, next, props.app!.global, taskId),
  );
  await afterTask(record);
}

async function openHomepage() {
  if (!props.app?.homepage) return;
  try {
    await openUrl(props.app.homepage);
  } catch {
    message.error("打开失败：无法打开链接");
  }
}

async function openDir() {
  if (!props.app) return;
  try {
    await openPath(props.app.app_dir);
  } catch {
    message.error("打开失败：无法打开目录");
  }
}
</script>

<style scoped>
.detail {
  padding: 16px;
  overflow: auto;
  height: 100%;
}
.head {
  display: flex;
  gap: 14px;
  align-items: center;
}
.title {
  font-size: 18px;
  font-weight: 600;
  display: flex;
  gap: 8px;
  align-items: center;
}
.sub {
  color: var(--text-dim);
  font-size: 13px;
  margin-top: 4px;
}
.update-line {
  color: #2080f0;
  font-size: 13px;
  margin-top: 4px;
}
.desc {
  margin: 14px 0 0;
  color: var(--text);
}
.actions {
  display: flex;
  gap: 8px;
  margin: 14px 0;
}
.fields {
  margin-top: 6px;
}
.section {
  margin-top: 16px;
}
.section-title {
  font-size: 13px;
  color: var(--text-dim);
  margin-bottom: 6px;
}
.tag {
  margin-right: 6px;
}
.persist {
  font-family: Consolas, monospace;
  font-size: 12px;
  color: var(--text);
}
.empty {
  color: var(--text-dim);
  text-align: center;
  padding-top: 60px;
}
</style>
