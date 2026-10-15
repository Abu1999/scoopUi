<template>
  <div class="page">
    <h3>运行环境</h3>
    <n-descriptions :column="1" size="small" bordered>
      <n-descriptions-item label="安装目录">
        {{ roots.env?.path ?? "未设置" }}
      </n-descriptions-item>
      <n-descriptions-item label="应用 / 软件源数量">
        {{ roots.env ? `${roots.env.app_count} / ${roots.env.bucket_count}` : "-" }}
      </n-descriptions-item>
    </n-descriptions>
    <div class="actions">
      <n-button size="small" :disabled="!roots.env" @click="checkEnv">检查环境（scoop --version）</n-button>
      <n-button size="small" :disabled="!roots.env || busy" :loading="moving" @click="moveEnv">修改位置</n-button>
      <n-popconfirm
        v-if="roots.env"
        :positive-button-props="{ disabled: busy, type: 'error' }"
        @positive-click="removeEnv"
      >
        <template #trigger>
          <n-button size="small" type="error" :disabled="busy">删除该环境</n-button>
        </template>
        确定删除 {{ roots.env.path }} 及其中的所有已安装应用？不可恢复。
      </n-popconfirm>
    </div>

    <h3>外观</h3>
    <div class="actions">
      <n-radio-group :value="appearance.theme" size="small" @update:value="onThemeChange">
        <n-radio-button value="system">跟随系统</n-radio-button>
        <n-radio-button value="light">浅色</n-radio-button>
        <n-radio-button value="dark">深色</n-radio-button>
      </n-radio-group>
    </div>

    <h3>配置文件</h3>
    <n-descriptions v-if="config" :column="1" size="small" bordered>
      <n-descriptions-item label="路径">{{ config.file_path }}</n-descriptions-item>
      <n-descriptions-item label="便携模式">{{ config.portable ? "是" : "否" }}</n-descriptions-item>
    </n-descriptions>
    <pre v-if="config" class="json">{{ configJson }}</pre>
    <div v-else class="empty">暂无配置信息</div>

    <n-alert v-if="mirrorError" type="error" size="small" class="mirror-error">
      <div>镜像信息加载失败：{{ mirrorError }}</div>
      <n-button size="tiny" class="retry" @click="loadMirror">重试</n-button>
    </n-alert>
    <MirrorSection v-if="mirror" :info="mirror" @changed="loadMirror" />
    <AccelerationSection v-if="mirror" :info="mirror" @changed="loadMirror" />
    <Aria2Section v-if="mirror" :info="mirror" @changed="loadMirror" />

    <n-modal
      v-model:show="showForceEnv"
      preset="card"
      title="环境被占用，无法删除"
      style="width: 480px; max-width: 92vw"
    >
      <div class="force-tip">以下程序占用了环境中的文件，强制删除将先结束这些进程：</div>
      <div v-for="h in envLock?.holders ?? []" :key="h.pid" class="force-row">
        <span class="force-name">{{ h.name || "未知进程" }}（PID {{ h.pid }}）</span>
        <span v-if="h.is_explorer" class="force-dim">资源管理器（结束后自动重启，桌面短暂闪烁）</span>
      </div>
      <div v-if="!envLock || envLock.holders.length === 0" class="force-row">
        <span class="force-name">未能识别占用进程（可能是资源管理器加载的扩展，强制删除会在必要时重启资源管理器）</span>
      </div>
      <div v-if="envLock && envLock.remaining_files.length" class="force-tip">
        另有 {{ envLock.remaining_files.length }} 个被占用文件需要清理。
      </div>
      <template #footer>
        <n-button size="small" :disabled="forcingEnv" @click="showForceEnv = false">取消</n-button>
        <n-button size="small" type="error" :loading="forcingEnv" @click="doForceRemoveEnv">
          强制删除
        </n-button>
      </template>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  NAlert,
  NButton,
  NDescriptions,
  NDescriptionsItem,
  NModal,
  NPopconfirm,
  NRadioButton,
  NRadioGroup,
  useMessage,
} from "naive-ui";
import { open } from "@tauri-apps/plugin-dialog";
import { envLockInfo, getMirrorInfo, getScoopConfig, runScoop } from "../api/commands";
import type { EnvLockInfo, MirrorInfo, ScoopConfig } from "../api/types";
import AccelerationSection from "../components/settings/AccelerationSection.vue";
import Aria2Section from "../components/settings/Aria2Section.vue";
import MirrorSection from "../components/settings/MirrorSection.vue";
import { useAppearanceStore } from "../stores/appearance";
import type { ThemePreference } from "../stores/appearance";
import { useAppsStore } from "../stores/apps";
import { useBucketsStore } from "../stores/buckets";
import { useLogsStore } from "../stores/logs";
import { useRootsStore } from "../stores/roots";
import { useUiStore } from "../stores/ui";

const roots = useRootsStore();
const apps = useAppsStore();
const buckets = useBucketsStore();
const logs = useLogsStore();
const ui = useUiStore();
const appearance = useAppearanceStore();
const message = useMessage();
const config = ref<ScoopConfig | null>(null);
const mirror = ref<MirrorInfo | null>(null);
const mirrorError = ref<string | null>(null);
const moving = ref(false);
const showForceEnv = ref(false);
const envLock = ref<EnvLockInfo | null>(null);
const forcingEnv = ref(false);
const busy = computed(() => logs.busy);
let mirrorSeq = 0;

async function moveEnv() {
  if (!roots.env) return;
  if (busy.value) {
    message.warning("当前有任务在运行，请先完成或取消后再操作环境");
    return;
  }
  const dir = await open({ directory: true, title: "选择新的 Scoop 安装目录" });
  if (typeof dir !== "string") return;
  moving.value = true;
  try {
    await roots.move(dir);
    message.success(`已迁移到 ${roots.env?.path ?? dir}`);
    await Promise.all([apps.load(), buckets.load()]);
  } catch (e) {
    message.error(String(e));
  } finally {
    moving.value = false;
  }
}

async function removeEnv() {
  if (busy.value) {
    message.warning("当前有任务在运行，请先完成或取消后再操作环境");
    return;
  }
  try {
    await roots.remove();
    message.success("已删除 Scoop 环境");
  } catch (e) {
    let lock: EnvLockInfo | null = null;
    try {
      lock = await envLockInfo();
    } catch {
      lock = null;
    }
    if (lock) {
      envLock.value = lock;
      showForceEnv.value = true;
    } else {
      message.error(String(e));
    }
  }
}

async function doForceRemoveEnv() {
  forcingEnv.value = true;
  try {
    const remaining = await roots.forceRemove();
    if (remaining.length === 0) {
      showForceEnv.value = false;
      message.success("已强制删除 Scoop 环境");
      await Promise.all([apps.load(), buckets.load()]);
    } else {
      message.warning(`仍有 ${remaining.length} 个条目未能删除，请稍后重试`);
    }
  } catch (e) {
    message.error(String(e));
  } finally {
    forcingEnv.value = false;
  }
}

async function onThemeChange(value: ThemePreference) {
  try {
    await appearance.setThemePreference(value);
  } catch (e) {
    message.error(String(e));
  }
}

const configJson = computed(() => (config.value ? JSON.stringify(config.value.values, null, 2) : ""));

async function loadConfig() {
  config.value = null;
  if (!roots.env) return;
  try {
    config.value = await getScoopConfig();
  } catch {
    config.value = null;
  }
}

async function loadMirror() {
  const seq = ++mirrorSeq;
  mirrorError.value = null;
  if (!roots.env) {
    mirror.value = null;
    return;
  }
  try {
    const info = await getMirrorInfo();
    if (seq !== mirrorSeq) return;
    mirror.value = info;
  } catch (e) {
    if (seq !== mirrorSeq) return;
    mirror.value = null;
    mirrorError.value = String(e);
  }
}

watch(
  () => roots.env?.path,
  () => {
    loadConfig();
    loadMirror();
  },
  { immediate: true },
);

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
  padding: 16px 16px 80px;
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
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
}
.json {
  margin-top: 10px;
  background: var(--bg-soft);
  border-radius: 4px;
  padding: 10px;
  font-size: 12px;
  overflow: auto;
  max-height: 240px;
}
.empty {
  color: var(--text-dim);
}
.mirror-error {
  margin-top: 12px;
}
.retry {
  margin-top: 6px;
}
.force-tip {
  color: var(--text-dim);
  font-size: 12px;
  margin-bottom: 8px;
  line-height: 1.6;
}
.force-row {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 6px 8px;
  margin-bottom: 6px;
  border: 1px solid var(--border);
  border-radius: 4px;
}
.force-name {
  font-size: 13px;
  font-family: Consolas, monospace;
}
.force-dim {
  color: var(--text-dim);
  font-size: 12px;
}
</style>
