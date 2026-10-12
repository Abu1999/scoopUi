<template>
  <div class="page">
    <div class="side">
      <div class="side-head">
        <n-button size="small" @click="showSave = true">另存当前状态</n-button>
        <n-button size="small" @click="doImport">导入</n-button>
      </div>
      <n-alert v-if="profiles.error" type="error" size="small" class="alert">{{ profiles.error }}</n-alert>
      <div class="list">
        <div v-if="profiles.list.length === 0" class="empty">从当前状态另存或导入配置集</div>
        <div
          v-for="p in profiles.list"
          :key="p.name"
          class="row"
          :class="{ active: p.name === profiles.selectedName }"
          @click="profiles.select(p.name)"
        >
          <div class="name">{{ p.name }}</div>
          <div class="meta">应用 {{ p.app_count }} · 桶 {{ p.bucket_count }}</div>
          <div class="meta">{{ formatTime(p.created) }}</div>
        </div>
      </div>
    </div>

    <div class="detail">
      <div v-if="!profiles.detail" class="empty">选择一个配置集查看详情</div>
      <template v-else>
        <div class="head">
          <div>
            <div class="title">{{ profiles.detail.name }}</div>
            <div class="sub">创建于 {{ formatTime(profiles.detail.created) }}</div>
          </div>
          <div class="actions">
            <n-button size="small" :disabled="busy" @click="openApply">应用</n-button>
            <n-button size="small" :disabled="busy" @click="doExport">导出</n-button>
            <n-button size="small" :disabled="busy" @click="openRename">重命名</n-button>
            <n-popconfirm
              :positive-button-props="{ disabled: busy, type: 'error' }"
              @positive-click="doDelete"
            >
              <template #trigger>
                <n-button size="small" type="error" :disabled="busy">删除</n-button>
              </template>
              确定删除配置集 {{ profiles.detail.name }}？
            </n-popconfirm>
          </div>
        </div>

        <div class="section">
          <div class="section-title">与当前状态差异</div>
          <div v-if="noDiff" class="hint">与当前状态一致</div>
          <template v-else>
            <div v-if="profiles.diff!.missing_buckets.length" class="diff-line">
              缺少桶：{{ profiles.diff!.missing_buckets.map((b) => b.name).join("、") }}
            </div>
            <div v-if="profiles.diff!.missing_apps.length" class="diff-line">
              缺少应用：{{ profiles.diff!.missing_apps.map(appLabel).join("、") }}
            </div>
            <div v-if="profiles.diff!.extra_apps.length" class="diff-line">
              多余应用（默认不卸载）：{{ profiles.diff!.extra_apps.map(appLabel).join("、") }}
            </div>
            <div v-if="profiles.diff!.extra_buckets.length" class="diff-line">
              多余桶（不处理）：{{ profiles.diff!.extra_buckets.join("、") }}
            </div>
            <div v-if="profiles.diff!.bucket_source_conflicts.length" class="diff-line">
              源冲突（不处理）：{{ profiles.diff!.bucket_source_conflicts.join("、") }}
            </div>
            <div v-if="profiles.diff!.settings.length" class="diff-line">
              设置变化：{{ profiles.diff!.settings.map((s) => s.key).join("、") }}
            </div>
          </template>
        </div>

        <div class="section">
          <div class="section-title">桶（{{ profiles.detail.buckets.length }}）</div>
          <div v-if="profiles.detail.buckets.length === 0" class="hint">无</div>
          <div v-for="b in profiles.detail.buckets" :key="b.name" class="line">
            <span class="mono">{{ b.name }}</span>
            <span class="dim">{{ b.source ?? "（无源）" }}</span>
          </div>
        </div>

        <div class="section">
          <div class="section-title">应用（{{ profiles.detail.apps.length }}）</div>
          <div v-if="profiles.detail.apps.length === 0" class="hint">无</div>
          <div v-for="a in profiles.detail.apps" :key="`${a.name}@${a.global}`" class="line">
            <span class="mono">{{ a.name }}</span>
            <span v-if="a.global" class="badge">全局</span>
            <span class="dim">{{ a.bucket ?? "（未知桶）" }}</span>
          </div>
        </div>

        <div class="section">
          <div class="section-title">下载加速设置</div>
          <div v-if="settingsEntries.length === 0" class="hint">无下载加速设置</div>
          <div v-for="[k, v] in settingsEntries" :key="k" class="line">
            <span class="mono">{{ k }}</span>
            <span class="dim">{{ formatValue(v) }}</span>
          </div>
        </div>
      </template>
    </div>

    <n-modal v-model:show="showSave" preset="card" title="另存当前状态" class="modal-sm">
      <n-input v-model:value="newName" placeholder="配置集名称" />
      <template #footer>
        <n-button size="small" @click="showSave = false">取消</n-button>
        <n-button size="small" type="primary" :disabled="!newName.trim()" @click="doSave">保存</n-button>
      </template>
    </n-modal>

    <n-modal v-model:show="showRename" preset="card" title="重命名配置集" class="modal-sm">
      <n-input v-model:value="renameName" placeholder="新名称" />
      <template #footer>
        <n-button size="small" @click="showRename = false">取消</n-button>
        <n-button size="small" type="primary" :disabled="!renameName.trim()" @click="doRename">确定</n-button>
      </template>
    </n-modal>

    <n-modal v-model:show="showImport" preset="card" title="导入配置集" class="modal-sm">
      <div class="hint">{{ importPath }}</div>
      <n-input v-model:value="importName" placeholder="配置集名称" />
      <template #footer>
        <n-button size="small" @click="showImport = false">取消</n-button>
        <n-button size="small" type="primary" :disabled="!importName.trim()" @click="doConfirmImport">
          确认导入
        </n-button>
      </template>
    </n-modal>

    <n-modal v-model:show="showApply" preset="card" title="应用配置集" class="modal">
      <div v-if="profiles.diff" class="hint">
        缺少桶 {{ profiles.diff.missing_buckets.length }} · 缺少应用 {{ profiles.diff.missing_apps.length }} ·
        多余应用 {{ profiles.diff.extra_apps.length }} · 设置变化 {{ profiles.diff.settings.length }}
      </div>
      <div v-for="s in profiles.steps" :key="s.key" class="step" :class="s.status">
        <span class="status">{{ statusText(s.status) }}</span>
        <span>{{ s.label }}</span>
      </div>
      <n-checkbox v-model:checked="uninstallExtra" :disabled="profiles.applying">同时卸载多余应用</n-checkbox>
      <n-alert v-if="profiles.applyError" type="error" size="small" class="alert">{{ profiles.applyError }}</n-alert>
      <template #footer>
        <n-button size="small" :disabled="profiles.applying" @click="showApply = false">关闭</n-button>
        <n-button size="small" type="primary" :loading="profiles.applying" @click="doApply">
          {{ profiles.applyError ? "重新计算并重试" : "开始执行" }}
        </n-button>
      </template>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { NAlert, NButton, NCheckbox, NInput, NModal, NPopconfirm, useMessage } from "naive-ui";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { ProfileApp } from "../api/types";
import { useLogsStore } from "../stores/logs";
import { useProfilesStore } from "../stores/profiles";
import type { ApplyStepStatus } from "../stores/profiles";

const profiles = useProfilesStore();
const logs = useLogsStore();
const message = useMessage();
const busy = computed(() => logs.busy);

const showSave = ref(false);
const newName = ref("");
const showRename = ref(false);
const renameName = ref("");
const showImport = ref(false);
const importPath = ref("");
const importName = ref("");
const showApply = ref(false);
const uninstallExtra = ref(false);

const settingsEntries = computed(() => Object.entries(profiles.detail?.settings ?? {}));

const noDiff = computed(() => {
  const d = profiles.diff;
  if (!d) return false;
  return (
    d.missing_buckets.length === 0 &&
    d.extra_buckets.length === 0 &&
    d.bucket_source_conflicts.length === 0 &&
    d.missing_apps.length === 0 &&
    d.extra_apps.length === 0 &&
    d.settings.length === 0
  );
});

function formatTime(sec: number): string {
  if (!sec) return "-";
  return new Date(sec * 1000).toLocaleString();
}

function formatValue(v: unknown): string {
  return typeof v === "string" ? v : JSON.stringify(v);
}

function appLabel(a: ProfileApp): string {
  return a.global ? `${a.name}（全局）` : a.name;
}

function statusText(s: ApplyStepStatus): string {
  const map: Record<ApplyStepStatus, string> = {
    pending: "待执行",
    running: "执行中",
    success: "成功",
    failed: "失败",
    skipped: "待执行",
  };
  return map[s];
}

async function doSave() {
  const name = newName.value.trim();
  if (!name) return;
  try {
    await profiles.saveFromCurrent(name);
    showSave.value = false;
    newName.value = "";
    message.success(`已保存配置集 ${name}`);
  } catch (e) {
    message.error(String(e));
  }
}

function openRename() {
  renameName.value = profiles.selectedName ?? "";
  showRename.value = true;
}

async function doRename() {
  const name = renameName.value.trim();
  if (!name) return;
  try {
    await profiles.rename(name);
    showRename.value = false;
    message.success("已重命名");
  } catch (e) {
    message.error(String(e));
  }
}

async function doDelete() {
  try {
    await profiles.remove();
    message.success("已删除");
  } catch (e) {
    message.error(String(e));
  }
}

async function doImport() {
  const path = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
  if (typeof path !== "string") return;
  importPath.value = path;
  importName.value = path.split(/[\\/]/).pop()?.replace(/\.json$/i, "") ?? "";
  showImport.value = true;
}

async function doConfirmImport() {
  const name = importName.value.trim();
  if (!name) return;
  try {
    await profiles.importFrom(importPath.value, name);
    showImport.value = false;
    message.success(`已导入配置集 ${name}`);
  } catch (e) {
    message.error(String(e));
  }
}

async function doExport() {
  if (!profiles.detail) return;
  const path = await save({
    defaultPath: `${profiles.detail.name}.json`,
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (!path) return;
  try {
    await profiles.exportTo(path);
    message.success("已导出");
  } catch (e) {
    message.error(String(e));
  }
}

async function openApply() {
  uninstallExtra.value = false;
  showApply.value = true;
  try {
    await profiles.refreshDiff();
  } catch (e) {
    message.error(String(e));
  }
}

async function doApply() {
  const ok = await profiles.apply(uninstallExtra.value);
  if (ok) {
    message.success("配置集应用完成");
    showApply.value = false;
  } else {
    message.error(profiles.applyError ?? "应用失败，详见任务日志");
  }
}
</script>

<style scoped>
.page {
  display: flex;
  height: 100%;
}
.side {
  width: 300px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-right: 1px solid #efeff5;
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
  border-bottom: 1px solid #f5f5f7;
}
.row:hover {
  background: #f5f7fa;
}
.row.active {
  background: #eaf3ff;
}
.row .name {
  font-size: 14px;
}
.row .meta {
  font-size: 12px;
  color: #888;
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
  color: #888;
  font-size: 12px;
  margin-top: 4px;
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
  color: #666;
  margin-bottom: 6px;
}
.diff-line {
  font-size: 13px;
  color: #444;
  margin-bottom: 4px;
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
  color: #888;
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
  color: #999;
  font-size: 12px;
}
.step {
  display: flex;
  gap: 8px;
  font-size: 13px;
  margin-bottom: 4px;
}
.step .status {
  width: 48px;
  flex-shrink: 0;
  color: #888;
}
.step.success .status {
  color: #18a058;
}
.step.failed .status {
  color: #d03050;
}
.step.running .status {
  color: #2080f0;
}
.empty {
  padding: 24px;
  color: #999;
  text-align: center;
}
.modal {
  width: 460px;
}
.modal-sm {
  width: 360px;
}
</style>
