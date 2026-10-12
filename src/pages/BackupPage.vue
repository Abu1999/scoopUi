<template>
  <div class="page">
    <div class="side">
      <div class="side-head">
        <n-button size="small" @click="doCreate">创建快照</n-button>
      </div>
      <n-alert v-if="snapshots.error" type="error" size="small" class="alert">{{ snapshots.error }}</n-alert>
      <div class="list">
        <div v-if="snapshots.list.length === 0" class="empty">还没有快照，点击「创建快照」保存当前状态</div>
        <div
          v-for="s in snapshots.list"
          :key="s.id"
          class="row"
          :class="{ active: s.id === snapshots.selectedId }"
          @click="snapshots.select(s.id)"
        >
          <div class="name">{{ formatTime(s.created) }}</div>
          <div class="meta">{{ s.root }}</div>
          <div class="meta">应用 {{ s.app_count }} · 软件源 {{ s.bucket_count }} · 配置集 {{ s.profile_count }}</div>
        </div>
      </div>
    </div>

    <div class="detail">
      <div v-if="!snapshots.selected" class="empty">选择一个快照查看详情</div>
      <template v-else>
        <div class="head">
          <div>
            <div class="title">{{ formatTime(snapshots.selected.created) }}</div>
            <div class="sub">来自根目录 {{ snapshots.selected.root }}</div>
          </div>
          <div class="actions">
            <n-button size="small" :disabled="busy" @click="doRestore">恢复配置与软件源</n-button>
            <n-popconfirm
              :positive-button-props="{ disabled: busy, type: 'error' }"
              @positive-click="doDelete"
            >
              <template #trigger>
                <n-button size="small" type="error" :disabled="busy">删除</n-button>
              </template>
              确定删除快照 {{ formatTime(snapshots.selected.created) }}？
            </n-popconfirm>
          </div>
        </div>

        <n-alert v-if="snapshots.plan && !snapshots.plan.root_matches" type="warning" size="small" class="alert">
          快照来自 {{ snapshots.plan.root }}，当前根目录不同；恢复将写入当前根目录的配置
        </n-alert>

        <div v-if="snapshots.plan" class="section">
          <div class="section-title">恢复计划</div>
          <div class="diff-line">
            配置文件：{{ snapshots.plan.config_present ? snapshots.plan.config_path : "快照未包含配置" }}
          </div>
          <div class="diff-line">
            重设软件源（{{ snapshots.plan.reset_buckets.length }}）：{{
              snapshots.plan.reset_buckets.map((b) => b.name).join("、") || "无"
            }}
          </div>
          <div class="diff-line">
            缺失软件源（{{ snapshots.plan.missing_buckets.length }}）：{{
              snapshots.plan.missing_buckets.map((b) => b.name).join("、") || "无"
            }}
          </div>
        </div>

        <div v-if="snapshots.plan" class="section">
          <div class="section-title">缺失应用（{{ snapshots.plan.missing_apps.length }}）</div>
          <div v-if="snapshots.plan.missing_apps.length === 0" class="hint">没有缺失应用</div>
          <div v-for="a in snapshots.plan.missing_apps" :key="`${a.name}@${a.global}`" class="line">
            <span class="mono">{{ a.name }}</span>
            <span v-if="a.global" class="badge">全局</span>
            <span class="dim">{{ a.version }} · {{ a.bucket ?? "未知软件源" }}</span>
            <n-button size="tiny" :disabled="busy" @click="doInstall(a)">安装</n-button>
          </div>
          <n-button
            v-if="snapshots.plan.missing_apps.length > 0"
            size="small"
            class="install-all"
            :disabled="busy"
            :loading="snapshots.restoring"
            @click="doInstallAll"
          >
            全部重装
          </n-button>
        </div>

        <div v-if="snapshots.steps.length > 0" class="section">
          <div class="section-title">执行进度</div>
          <div v-for="s in snapshots.steps" :key="s.key" class="step" :class="s.status">
            <span class="status-text">{{ statusText(s.status) }}</span>
            <span>{{ s.label }}</span>
          </div>
        </div>
        <n-alert v-if="snapshots.restoreError" type="error" size="small" class="alert">
          {{ snapshots.restoreError }}
        </n-alert>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from "vue";
import { NAlert, NButton, NPopconfirm, useMessage } from "naive-ui";
import type { SnapshotApp } from "../api/types";
import { useLogsStore } from "../stores/logs";
import { useSnapshotsStore } from "../stores/snapshots";
import type { InstallStepStatus } from "../stores/snapshots";

const snapshots = useSnapshotsStore();
const logs = useLogsStore();
const message = useMessage();
const busy = computed(() => logs.busy);

onMounted(() => {
  snapshots.load();
});

function formatTime(sec: number): string {
  if (!sec) return "-";
  return new Date(sec * 1000).toLocaleString();
}

function statusText(s: InstallStepStatus): string {
  const map: Record<InstallStepStatus, string> = {
    pending: "待执行",
    running: "执行中",
    success: "成功",
    failed: "失败",
    skipped: "待执行",
  };
  return map[s];
}

async function doCreate() {
  try {
    await snapshots.create();
    message.success("已创建快照");
  } catch (e) {
    message.error(String(e));
  }
}

async function doDelete() {
  try {
    await snapshots.remove();
    message.success("已删除快照");
  } catch (e) {
    message.error(String(e));
  }
}

async function doRestore() {
  const ok = await snapshots.restoreConfigAndBuckets();
  if (ok) {
    message.success("配置与软件源已恢复");
  } else {
    message.error(snapshots.restoreError ?? "恢复失败，详见任务日志");
  }
}

async function doInstall(app: SnapshotApp) {
  const ok = await snapshots.installApp(app);
  if (ok) {
    message.success(`已安装 ${app.name}`);
  } else {
    message.error(snapshots.restoreError ?? "安装失败，详见任务日志");
  }
}

async function doInstallAll() {
  const ok = await snapshots.installAllMissing();
  if (ok) {
    message.success("缺失应用已全部安装");
  } else {
    message.error(snapshots.restoreError ?? "安装失败，详见任务日志");
  }
}
</script>

<style scoped>
.page {
  display: flex;
  height: 100%;
}
.side {
  width: 320px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-right: 1px solid var(--border);
}
.side-head {
  display: flex;
  gap: 8px;
  padding: 10px;
}
.alert {
  margin: 0 10px 8px;
}
.list {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.row {
  padding: 8px 12px;
  cursor: pointer;
  border-bottom: 1px solid var(--border);
}
.row:hover {
  background: var(--bg-hover);
}
.row.active {
  background: var(--bg-active);
}
.row .name {
  font-size: 14px;
}
.row .meta {
  font-size: 12px;
  color: var(--text-dim);
  word-break: break-all;
}
.detail {
  flex: 1;
  min-width: 0;
  padding: 16px;
  overflow: auto;
}
.head {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 12px;
}
.title {
  font-size: 18px;
  font-weight: 600;
}
.sub {
  color: var(--text-dim);
  font-size: 12px;
  margin-top: 4px;
  word-break: break-all;
}
.actions {
  display: flex;
  gap: 8px;
}
.section {
  margin-top: 18px;
}
.section-title {
  font-size: 13px;
  color: var(--text-dim);
  margin-bottom: 6px;
}
.diff-line {
  font-size: 13px;
  color: var(--text);
  margin-bottom: 4px;
  word-break: break-all;
}
.line {
  display: flex;
  gap: 8px;
  align-items: center;
  font-size: 13px;
  margin-bottom: 4px;
}
.mono {
  font-family: Consolas, monospace;
}
.dim {
  color: var(--text-dim);
  font-size: 12px;
  word-break: break-all;
}
.badge {
  font-size: 11px;
  color: #18a058;
  border: 1px solid #18a058;
  border-radius: 3px;
  padding: 0 4px;
}
.hint {
  color: var(--text-dim);
  font-size: 12px;
}
.install-all {
  margin-top: 8px;
}
.step {
  display: flex;
  gap: 8px;
  font-size: 13px;
  margin-bottom: 4px;
}
.step .status-text {
  width: 48px;
  flex-shrink: 0;
  color: var(--text-dim);
}
.step.success .status-text {
  color: #18a058;
}
.step.failed .status-text {
  color: #d03050;
}
.step.running .status-text {
  color: #2080f0;
}
.empty {
  padding: 24px;
  color: var(--text-dim);
  text-align: center;
}
</style>
