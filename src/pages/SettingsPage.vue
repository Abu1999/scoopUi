<template>
  <div class="page">
    <h3>运行环境</h3>
    <n-descriptions :column="1" size="small" bordered>
      <n-descriptions-item label="当前根目录">
        {{ roots.active?.path ?? "未设置" }}
      </n-descriptions-item>
      <n-descriptions-item label="应用 / 软件源数量">
        {{ roots.active ? `${roots.active.app_count} / ${roots.active.bucket_count}` : "-" }}
      </n-descriptions-item>
      <n-descriptions-item label="全局目录">
        {{ roots.active?.global_dir ?? "-" }}
      </n-descriptions-item>
    </n-descriptions>
    <div class="actions">
      <n-button size="small" :disabled="!roots.active" @click="checkEnv">检查环境（scoop --version）</n-button>
    </div>

    <n-alert v-if="boot?.boot_root" type="info" size="small" class="boot-alert">
      <div>由 ScoopUI 初始化的 Scoop 环境：{{ boot.boot_root }}</div>
      <div class="boot-hint">卸载本应用时安装程序会询问是否连同删除；也可在此立即删除（会一并删除其中已安装的应用）。</div>
      <n-popconfirm :positive-button-props="{ type: 'error' }" @positive-click="removeEnv">
        <template #trigger>
          <n-button size="tiny" type="error">删除该环境</n-button>
        </template>
        确定删除 {{ boot.boot_root }} 及其中的所有已安装应用？不可恢复。
      </n-popconfirm>
    </n-alert>

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
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  NAlert,
  NButton,
  NDescriptions,
  NDescriptionsItem,
  NPopconfirm,
  NRadioButton,
  NRadioGroup,
  useMessage,
} from "naive-ui";
import {
  getBootInfo,
  getMirrorInfo,
  getScoopConfig,
  removeBootstrappedScoop,
  runScoop,
} from "../api/commands";
import type { BootInfo, MirrorInfo, ScoopConfig } from "../api/types";
import AccelerationSection from "../components/settings/AccelerationSection.vue";
import Aria2Section from "../components/settings/Aria2Section.vue";
import MirrorSection from "../components/settings/MirrorSection.vue";
import { useAppearanceStore } from "../stores/appearance";
import type { ThemePreference } from "../stores/appearance";
import { useAppsStore } from "../stores/apps";
import { useLogsStore } from "../stores/logs";
import { useRootsStore } from "../stores/roots";
import { useUiStore } from "../stores/ui";

const roots = useRootsStore();
const logs = useLogsStore();
const ui = useUiStore();
const appearance = useAppearanceStore();
const message = useMessage();
const config = ref<ScoopConfig | null>(null);
const mirror = ref<MirrorInfo | null>(null);
const mirrorError = ref<string | null>(null);
const boot = ref<BootInfo | null>(null);
let mirrorSeq = 0;

async function loadBoot() {
  try {
    boot.value = await getBootInfo();
  } catch {
    boot.value = null;
  }
}

async function removeEnv() {
  try {
    const removed = await removeBootstrappedScoop();
    message.success(`已删除 Scoop 环境：${removed}`);
    boot.value = null;
    await Promise.all([roots.load(), useAppsStore().load()]);
  } catch (e) {
    message.error(String(e));
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
  if (!roots.active) return;
  try {
    config.value = await getScoopConfig();
  } catch {
    config.value = null;
  }
}

async function loadMirror() {
  const seq = ++mirrorSeq;
  mirrorError.value = null;
  if (!roots.active) {
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
  () => roots.active?.path,
  () => {
    loadConfig();
    loadMirror();
    loadBoot();
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
.boot-alert {
  margin-top: 12px;
}
.boot-hint {
  color: var(--text-dim);
  font-size: 12px;
  margin: 4px 0 6px;
}
.retry {
  margin-top: 6px;
}
</style>
