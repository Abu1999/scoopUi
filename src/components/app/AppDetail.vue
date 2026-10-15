<template>
  <div class="detail">
    <div v-if="!app" class="empty">选择一个应用查看详情</div>
    <template v-else>
      <div class="head">
        <AppIcon :name="app.name" :size="64" :source="app.icon ?? app.homepage" />
        <div>
          <div class="title">
            {{ app.name }}
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
        <n-button size="small" @click="doUpdate">更新</n-button>
        <n-popconfirm
          :positive-button-props="{ type: 'error' }"
          @positive-click="doUninstall"
        >
          <template #trigger>
            <n-button size="small" type="error">卸载</n-button>
          </template>
          确定卸载 {{ app.name }}？程序文件将被删除（持久化数据保留）。
        </n-popconfirm>
        <n-button size="small" @click="toggleHold">
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

    <n-modal
      v-model:show="showForceUninstall"
      preset="card"
      title="应用未能完全卸载"
      style="width: 480px; max-width: 92vw"
    >
      <div class="force-tip">
        卸载未完成（可能应用仍在运行，或文件被占用，如资源管理器加载的扩展）。
        强制卸载将结束相关进程（必要时自动重启资源管理器）并清理残留文件：
      </div>
      <div v-for="p in appProcs" :key="p.pid" class="force-row">
        <span class="force-name">{{ p.name || "未知进程" }}（PID {{ p.pid }}）</span>
      </div>
      <div v-if="appProcs.length === 0" class="force-row">
        <span class="force-name">未检测到正在运行的进程（可能是文件被其他程序占用）</span>
      </div>
      <template #footer>
        <n-button size="small" :disabled="forcingUninstall" @click="showForceUninstall = false">
          取消
        </n-button>
        <n-button size="small" type="error" :loading="forcingUninstall" @click="doForceUninstall">
          强制卸载
        </n-button>
      </template>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { NButton, NDescriptions, NDescriptionsItem, NModal, NPopconfirm, NTag, useMessage } from "naive-ui";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import AppIcon from "./AppIcon.vue";
import type { AppProcessInfo, InstalledApp } from "../../api/types";
import {
  forceUninstallPackage,
  listAppProcesses,
  setHold,
  uninstallPackage,
  updatePackage,
} from "../../api/commands";
import { useAppsStore } from "../../stores/apps";
import { useRootsStore } from "../../stores/roots";
import { useUiStore } from "../../stores/ui";
import { runTask } from "../../utils/tasks";
import type { TaskRecord } from "../../stores/logs";

const props = defineProps<{ app: InstalledApp | null }>();
const message = useMessage();
const apps = useAppsStore();
const roots = useRootsStore();
const ui = useUiStore();
const update = computed(() => (props.app ? apps.updateFor(props.app) : null));
const showForceUninstall = ref(false);
const appProcs = ref<AppProcessInfo[]>([]);
const forcingUninstall = ref(false);
const pendingUninstall = ref<string | null>(null);
/** 本组件操作重入保护（防双击重复发起；跨应用并发不受影响） */
const acting = ref(false);

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
  if (acting.value) return;
  if (!props.app) return;
  const name = props.app.name;
  ui.logDrawerOpen = true;
  acting.value = true;
  try {
    const record = await runTask(`更新 ${name}`, (taskId) => updatePackage(name, false, taskId));
    await afterTask(record);
  } finally {
    acting.value = false;
  }
}

async function doUninstall() {
  if (acting.value) return;
  if (!props.app) return;
  const name = props.app.name;
  ui.logDrawerOpen = true;
  acting.value = true;
  try {
    const record = await runTask(`卸载 ${name}`, (taskId) => uninstallPackage(name, false, taskId));
    await afterTask(record);
    if (!record.success) return;
    // Scoop 检测到进程在运行/文件被占用时会打印警告但退出码仍为 0，
    // 因此以「应用是否真的从列表消失」判断，未消失则引导强制卸载。
    if (!apps.apps.some((a) => a.name.toLowerCase() === name.toLowerCase())) return;
    try {
      appProcs.value = await listAppProcesses(name);
    } catch {
      appProcs.value = [];
    }
    pendingUninstall.value = name;
    showForceUninstall.value = true;
  } finally {
    acting.value = false;
  }
}

async function doForceUninstall() {
  const name = pendingUninstall.value;
  if (!name || forcingUninstall.value) return;
  forcingUninstall.value = true;
  ui.logDrawerOpen = true;
  try {
    const record = await runTask(`强制卸载 ${name}`, (taskId) =>
      forceUninstallPackage(name, false, taskId),
    );
    await afterTask(record);
    if (!record.success) return;
    if (!apps.apps.some((a) => a.name.toLowerCase() === name.toLowerCase())) {
      showForceUninstall.value = false;
      pendingUninstall.value = null;
      message.success(`已强制卸载 ${name}`);
    } else {
      message.warning(`仍有文件被占用，未能完全删除 ${name}；可重启资源管理器后重试`);
    }
  } finally {
    forcingUninstall.value = false;
  }
}

async function toggleHold() {
  if (acting.value) return;
  if (!props.app) return;
  const name = props.app.name;
  const next = !props.app.hold;
  ui.logDrawerOpen = true;
  acting.value = true;
  try {
    const record = await runTask(`${next ? "锁定" : "解除锁定"} ${name}`, (taskId) =>
      setHold(name, next, taskId),
    );
    await afterTask(record);
  } finally {
    acting.value = false;
  }
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
.force-tip {
  color: var(--text-dim);
  font-size: 12px;
  margin-bottom: 8px;
  line-height: 1.6;
}
.force-row {
  padding: 6px 8px;
  margin-bottom: 6px;
  border: 1px solid var(--border);
  border-radius: 4px;
}
.force-name {
  font-size: 13px;
  font-family: Consolas, monospace;
}
</style>
