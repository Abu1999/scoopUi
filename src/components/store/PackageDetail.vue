<template>
  <div class="detail">
    <div v-if="!pkg" class="empty">选择一个包查看详情</div>
    <template v-else>
      <div class="head">
        <AppIcon :name="pkg.name" :size="64" :source="pkg.icon ?? pkg.homepage" />
        <div>
          <div class="title">
            {{ pkg.name }}
            <n-tag v-if="pkg.deprecated" size="small" type="warning">已弃用</n-tag>
          </div>
          <div class="sub">
            版本 {{ pkg.version }} · 来自软件源 {{ pkg.bucket }}<span v-if="pkg.license"> · {{ pkg.license }}</span>
          </div>
        </div>
      </div>

      <p v-if="pkg.description" class="desc">{{ pkg.description }}</p>

      <div class="actions">
        <n-button v-if="pkg.homepage" size="small" @click="openHomepage">打开主页</n-button>
      </div>

      <n-descriptions :column="1" size="small" label-placement="left" class="fields">
        <n-descriptions-item v-if="pkg.homepage" label="主页">{{ pkg.homepage }}</n-descriptions-item>
        <n-descriptions-item v-if="pkg.license" label="许可证">{{ pkg.license }}</n-descriptions-item>
      </n-descriptions>

      <div class="install">
        <div class="install-row">
          <n-input v-model:value="version" size="small" placeholder="版本（留空 = 最新）" class="version" />
          <n-checkbox v-model:checked="globalInstall">全局安装</n-checkbox>
        </div>
        <div class="install-row">
          <n-button type="primary" size="small" :disabled="busy" @click="doInstall">安装</n-button>
          <span class="install-source">
            来自软件源 {{ pkg.bucket }}<span v-if="globalInstall">；全局安装可能需要管理员权限</span>
          </span>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import {
  NButton,
  NCheckbox,
  NDescriptions,
  NDescriptionsItem,
  NInput,
  NTag,
  useMessage,
} from "naive-ui";
import { openUrl } from "@tauri-apps/plugin-opener";
import AppIcon from "../app/AppIcon.vue";
import type { PackageEntry } from "../../api/types";
import { installPackage } from "../../api/commands";
import { useAppsStore } from "../../stores/apps";
import { useLogsStore } from "../../stores/logs";
import { useRootsStore } from "../../stores/roots";
import { useUiStore } from "../../stores/ui";
import { runTask } from "../../utils/tasks";
import { registerInstall } from "../../utils/postinstall";

const props = defineProps<{ pkg: PackageEntry | null }>();
const message = useMessage();
const apps = useAppsStore();
const logs = useLogsStore();
const roots = useRootsStore();
const ui = useUiStore();
const version = ref("");
const globalInstall = ref(true);
const busy = computed(() => logs.busy);

async function doInstall() {
  if (busy.value) return;
  if (!props.pkg) return;
  const pkg = props.pkg;
  const ver = version.value.trim() === "" ? null : version.value.trim();
  ui.logDrawerOpen = true;
  const record = await runTask(`安装 ${pkg.name}`, (taskId) =>
    installPackage(pkg.name, pkg.bucket, ver, globalInstall.value, taskId),
  );
  if (record.success) {
    const registered = await registerInstall(pkg.name, globalInstall.value);
    message.success(
      registered.length > 0 ? `已安装 ${pkg.name}（已注册右键菜单/文件关联）` : `已安装 ${pkg.name}`,
    );
    await Promise.all([apps.load(), roots.load()]);
  } else {
    message.error(
      record.hint
        ? `安装失败（退出码 ${record.code ?? "未知"}）：${record.hint}`
        : `安装失败（退出码 ${record.code ?? "未知"}），详见任务日志`,
    );
  }
}

async function openHomepage() {
  if (!props.pkg?.homepage) return;
  try {
    await openUrl(props.pkg.homepage);
  } catch {
    message.error("打开失败：无法打开链接");
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
.install {
  margin-top: 20px;
  padding-top: 12px;
  border-top: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.install-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.version {
  width: 180px;
}
.install-source {
  color: var(--text-dim);
  font-size: 12px;
}
.empty {
  color: var(--text-dim);
  text-align: center;
  padding-top: 60px;
}
</style>
