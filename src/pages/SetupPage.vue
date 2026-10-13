<template>
  <div class="page">
    <div class="card">
      <h2 class="title">初始化 Scoop</h2>
      <p class="desc">未检测到 Scoop。可以使用内置快照离线初始化，或在线安装最新版。</p>
      <p v-if="bundle" class="snapshot">
        内置快照：Scoop @{{ bundle.scoop_commit.slice(0, 7) }} · Main @{{ bundle.main_commit.slice(0, 7) }} ·
        {{ bundle.built }}
      </p>

      <n-alert v-if="report && report.existing_root" type="info" size="small" class="alert">
        <div>检测到已有 Scoop：{{ report.existing_root }}</div>
        <n-button size="tiny" class="retry" @click="useExisting(report.existing_root)">直接使用</n-button>
      </n-alert>

      <n-descriptions v-if="report" :column="2" size="small" bordered class="env">
        <n-descriptions-item label="PowerShell">
          {{ report.powershell_ok ? "可用" : "不可用（需要 5.1+）" }}
        </n-descriptions-item>
        <n-descriptions-item label="解压工具">
          {{ report.tar_available ? "tar（系统自带）" : "PowerShell Expand-Archive" }}
        </n-descriptions-item>
        <n-descriptions-item label="git">
          {{ report.git_available ? "已安装" : "未安装（仅影响更新软件源）" }}
        </n-descriptions-item>
        <n-descriptions-item label="权限">{{ report.elevated ? "管理员" : "普通用户" }}</n-descriptions-item>
      </n-descriptions>

      <div class="field">
        <span class="label">安装目录</span>
        <n-input
          v-model:value="dir"
          size="small"
          class="dir-input"
          placeholder="默认 %USERPROFILE%\scoop"
          @blur="refreshCheck"
          @keyup.enter="refreshCheck"
        />
        <n-button size="small" @click="pickDir">选择</n-button>
      </div>
      <div v-if="report && !report.dir_ok" class="hint error">{{ report.dir_reason }}</div>

      <n-checkbox v-model:checked="addToPath" class="path-toggle">
        创建 scoop 命令并加入 PATH（新终端可直接使用 scoop）
      </n-checkbox>

      <div class="actions">
        <n-button type="primary" :disabled="!canInit" :loading="busy" @click="doInit">
          一键初始化（内置，无需下载）
        </n-button>
      </div>

      <div class="advanced">
        <div class="section-title">或在线安装最新版</div>
        <div class="online">
          <n-input v-model:value="proxy" size="small" placeholder="HTTP 代理（可选，host:port）" class="proxy" />
          <n-button size="small" :disabled="!canInit" :loading="busy" @click="doOnline">在线安装</n-button>
        </div>
      </div>

      <n-collapse class="manual">
        <n-collapse-item title="手动安装说明" name="manual">
          <pre class="cmd">Set-ExecutionPolicy RemoteSigned -Scope CurrentUser
irm get.scoop.sh | iex</pre>
        </n-collapse-item>
      </n-collapse>

      <div class="footer">
        <n-button text size="small" @click="useExisting()">已经装过 Scoop？选择根目录</n-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  NAlert,
  NButton,
  NCheckbox,
  NCollapse,
  NCollapseItem,
  NDescriptions,
  NDescriptionsItem,
  NInput,
  useMessage,
} from "naive-ui";
import { open } from "@tauri-apps/plugin-dialog";
import {
  addRoot,
  checkScoopPrereqs,
  extractScoopBundle,
  finalizeScoopBootstrap,
  getScoopBundleInfo,
  installScoopOnline,
  setActiveRoot,
} from "../api/commands";
import type { BundleInfo, PrereqReport } from "../api/types";
import { useAppsStore } from "../stores/apps";
import { useBucketsStore } from "../stores/buckets";
import { useLogsStore } from "../stores/logs";
import type { TaskRecord } from "../stores/logs";
import { useRootsStore } from "../stores/roots";
import { useUiStore } from "../stores/ui";
import { runTask } from "../utils/tasks";

const message = useMessage();
const roots = useRootsStore();
const apps = useAppsStore();
const buckets = useBucketsStore();
const logs = useLogsStore();
const ui = useUiStore();

const report = ref<PrereqReport | null>(null);
const bundle = ref<BundleInfo | null>(null);
const dir = ref("");
const proxy = ref("");
const addToPath = ref(true);
const busy = computed(() => logs.busy);
const canInit = computed(() => !!report.value?.dir_ok && !busy.value);

function failMessage(label: string, record: TaskRecord): string {
  return record.hint
    ? `${label}失败（退出码 ${record.code ?? "未知"}）：${record.hint}`
    : `${label}失败（退出码 ${record.code ?? "未知"}），详见任务日志`;
}

async function refreshCheck() {
  try {
    report.value = await checkScoopPrereqs(dir.value.trim());
    if (!dir.value.trim() && report.value?.default_dir) {
      dir.value = report.value.default_dir;
    }
  } catch (e) {
    message.error(String(e));
  }
}

onMounted(async () => {
  try {
    bundle.value = await getScoopBundleInfo();
  } catch {
    bundle.value = null;
  }
  await refreshCheck();
});

async function pickDir() {
  const picked = await open({ directory: true, title: "选择 Scoop 安装目录" });
  if (typeof picked !== "string") return;
  dir.value = picked;
  await refreshCheck();
}

async function finish(target: string) {
  await addRoot(target);
  await setActiveRoot(target);
  await Promise.all([roots.load(), apps.load(), buckets.load()]);
  message.success("Scoop 已就绪");
  ui.setPage("installed");
}

async function doInit() {
  const target = dir.value.trim();
  if (!target || !canInit.value) return;
  ui.logDrawerOpen = true;
  const record = await runTask("初始化 Scoop（内置）", (taskId) => extractScoopBundle(target, taskId));
  if (!record.success) {
    message.error(failMessage("初始化", record));
    return;
  }
  try {
    await finalizeScoopBootstrap(target, addToPath.value);
  } catch (e) {
    message.error(String(e));
    return;
  }
  await finish(target);
}

async function doOnline() {
  const target = dir.value.trim();
  if (!target || !canInit.value) return;
  ui.logDrawerOpen = true;
  const proxyValue = proxy.value.trim() === "" ? null : proxy.value.trim();
  const record = await runTask("安装 Scoop（在线）", (taskId) =>
    installScoopOnline(target, proxyValue, taskId),
  );
  if (!record.success) {
    message.error(failMessage("安装", record));
    return;
  }
  await finish(target);
}

async function useExisting(existing?: string) {
  let target: string | null = existing ?? null;
  if (!target) {
    const picked = await open({ directory: true, title: "选择已有的 Scoop 根目录" });
    if (typeof picked !== "string") return;
    target = picked;
  }
  try {
    await finish(target);
  } catch (e) {
    message.error(String(e));
  }
}
</script>

<style scoped>
.page {
  height: 100%;
  overflow: auto;
  display: flex;
  justify-content: center;
  padding: 32px 16px;
}
.card {
  width: 640px;
  max-width: 100%;
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 20px 24px;
  align-self: flex-start;
}
.title {
  margin: 0 0 8px;
  font-size: 18px;
}
.desc {
  margin: 0 0 6px;
  color: var(--text-dim);
  font-size: 13px;
}
.snapshot {
  margin: 0 0 14px;
  color: var(--text-dim);
  font-size: 12px;
}
.alert {
  margin-bottom: 12px;
}
.retry {
  margin-top: 6px;
}
.env {
  margin-bottom: 14px;
}
.field {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
.label {
  color: var(--text-dim);
  font-size: 13px;
  flex-shrink: 0;
}
.dir-input {
  flex: 1;
  min-width: 0;
}
.hint {
  color: var(--text-dim);
  font-size: 12px;
  margin-bottom: 8px;
}
.hint.error {
  color: #d03050;
}
.path-toggle {
  margin-bottom: 14px;
}
.actions {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}
.advanced {
  margin-bottom: 6px;
}
.section-title {
  font-size: 13px;
  color: var(--text-dim);
  margin-bottom: 6px;
}
.online {
  display: flex;
  gap: 8px;
  align-items: center;
}
.proxy {
  width: 240px;
}
.cmd {
  margin: 0;
  padding: 8px 10px;
  background: var(--bg-soft);
  border-radius: 4px;
  font-family: Consolas, monospace;
  font-size: 12px;
  color: var(--text);
}
.footer {
  margin-top: 14px;
}
</style>
