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
            版本 {{ pkg.version }} · 来自 {{ pkg.bucket }} 桶<span v-if="pkg.license"> · {{ pkg.license }}</span>
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

      <div class="install-hint">安装功能将在后续版本提供</div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { NButton, NDescriptions, NDescriptionsItem, NTag, useMessage } from "naive-ui";
import { openUrl } from "@tauri-apps/plugin-opener";
import AppIcon from "../app/AppIcon.vue";
import type { PackageEntry } from "../../api/types";

const props = defineProps<{ pkg: PackageEntry | null }>();
const message = useMessage();

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
  color: #888;
  font-size: 13px;
  margin-top: 4px;
}
.desc {
  margin: 14px 0 0;
  color: #444;
}
.actions {
  display: flex;
  gap: 8px;
  margin: 14px 0;
}
.fields {
  margin-top: 6px;
}
.install-hint {
  margin-top: 20px;
  padding-top: 12px;
  border-top: 1px solid #efeff5;
  color: #999;
  font-size: 12px;
}
.empty {
  color: #999;
  text-align: center;
  padding-top: 60px;
}
</style>
