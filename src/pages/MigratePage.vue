<template>
  <div class="page">
    <div class="toolbar">
      <n-button size="small" :loading="migrate.loading" @click="doScan">扫描本机已安装软件</n-button>
      <n-checkbox v-model:checked="migrate.showManaged">显示已管理条目</n-checkbox>
      <n-checkbox v-model:checked="uninstallFirst">安装前先卸载旧版</n-checkbox>
      <span v-if="migrate.scanned" class="count">
        共 {{ migrate.entries.length }} 条 · 已由 scoop 管理 {{ migrate.managedCount }} 条 · 可接管
        {{ migrate.matchedCount }} 条
      </span>
    </div>

    <n-alert v-if="migrate.error" type="error" size="small" class="alert">{{ migrate.error }}</n-alert>
    <div v-if="!migrate.scanned && !migrate.error" class="hint">点击「扫描本机已安装软件」开始</div>
    <div v-else-if="groups.length === 0" class="hint">没有可迁移的条目</div>
    <div v-else class="groups">
      <div v-for="g in groups" :key="g.key" class="group">
        <div class="group-title">{{ g.title }}（{{ g.entries.length }}）</div>
        <div v-for="e in g.entries" :key="e.app.key_path" class="row">
          <div class="info">
            <div class="name">
              {{ e.app.name }}
              <span v-if="e.app.managed" class="badge">已管理</span>
            </div>
            <div class="meta">
              <span v-if="e.app.version">{{ e.app.version }}</span>
              <span v-if="e.app.publisher"> · {{ e.app.publisher }}</span>
              <span v-if="e.app.install_location"> · {{ e.app.install_location }}</span>
            </div>
          </div>
          <div class="match">
            <template v-if="e.matches.length > 0">
              <n-tag size="small" type="success">{{ matchLabel(e.matches[0]) }}</n-tag>
              <span v-if="e.matches.length > 1" class="more">等 {{ e.matches.length }} 个</span>
            </template>
            <span v-else class="hint">未匹配到软件包</span>
          </div>
          <div class="actions">
            <n-button size="tiny" :disabled="busy || e.app.managed || e.matches.length === 0" @click="doInstall(e)">安装</n-button>
            <n-button size="tiny" :disabled="e.matches.length === 0" @click="doView(e)">查看</n-button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { NAlert, NButton, NCheckbox, NTag, useMessage } from "naive-ui";
import type { MigrationEntry, MigrationMatch } from "../api/types";
import { useLogsStore } from "../stores/logs";
import { useMigrateStore } from "../stores/migrate";
import { useUiStore } from "../stores/ui";

interface Group {
  key: string;
  title: string;
  entries: MigrationEntry[];
}

const migrate = useMigrateStore();
const logs = useLogsStore();
const message = useMessage();
const busy = computed(() => logs.busy);
const uninstallFirst = ref(false);

const groups = computed<Group[]>(() => {
  const result: Group[] = [
    { key: "high", title: "高置信匹配", entries: [] },
    { key: "medium", title: "中置信匹配", entries: [] },
    { key: "low", title: "低置信匹配", entries: [] },
    { key: "none", title: "未匹配", entries: [] },
  ];
  for (const e of migrate.visible) {
    const best = e.matches[0]?.confidence;
    const key = best === "high" || best === "medium" || best === "low" ? best : "none";
    result.find((g) => g.key === key)!.entries.push(e);
  }
  return result.filter((g) => g.entries.length > 0);
});

const confidenceLabel: Record<string, string> = {
  high: "高置信",
  medium: "中置信",
  low: "低置信",
};

function matchLabel(match: MigrationMatch): string {
  return `${match.bucket}/${match.name}（${confidenceLabel[match.confidence] ?? match.confidence}）`;
}

async function doScan() {
  if (busy.value) return;
  await migrate.scan();
}

async function doInstall(entry: MigrationEntry) {
  const match = entry.matches[0];
  if (!match || busy.value) return;
  const result = await migrate.install(entry, match, uninstallFirst.value);
  if (result.ok) {
    if (result.warning) {
      message.warning(result.warning);
    } else {
      message.success(`已安装 ${match.name}`);
    }
  } else {
    message.error(result.error ?? "安装失败，详见任务日志");
  }
}

function doView(entry: MigrationEntry) {
  const match = entry.matches[0];
  if (!match) return;
  useUiStore().searchInStore(match.name);
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
  gap: 12px;
  margin-bottom: 12px;
  flex-wrap: wrap;
}
.count {
  color: var(--text-dim);
  font-size: 12px;
}
.alert {
  margin-bottom: 12px;
}
.hint {
  color: var(--text-dim);
  font-size: 13px;
  text-align: center;
  padding: 40px 0;
}
.group {
  margin-bottom: 18px;
}
.group-title {
  font-size: 13px;
  color: var(--text-dim);
  margin-bottom: 6px;
}
.row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 10px;
  border-bottom: 1px solid var(--border);
}
.info {
  flex: 1;
  min-width: 0;
}
.name {
  font-size: 14px;
}
.meta {
  font-size: 12px;
  color: var(--text-dim);
  word-break: break-all;
}
.match {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}
.more {
  font-size: 12px;
  color: var(--text-dim);
}
.actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
.badge {
  margin-left: 6px;
  font-size: 11px;
  color: #18a058;
  border: 1px solid #18a058;
  border-radius: 3px;
  padding: 0 4px;
}
</style>
