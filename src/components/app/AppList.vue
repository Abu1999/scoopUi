<template>
  <div class="list">
    <div v-if="apps.length === 0" class="empty">没有已安装的应用</div>
    <div
      v-for="app in apps"
      :key="appKey(app)"
      class="row"
      :class="{ active: appKey(app) === selected }"
      @click="$emit('select', appKey(app))"
    >
      <AppIcon :name="app.name" :size="28" :source="app.icon ?? app.homepage" />
      <div class="info">
        <div class="name">
          {{ app.name }}
          <span v-if="app.hold" class="hold">锁定</span>
          <span v-if="updates[appKey(app)]" class="update">可更新</span>
        </div>
        <div class="meta">
          {{ app.version }}
          <span v-if="app.bucket"> · {{ app.bucket }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import AppIcon from "./AppIcon.vue";
import type { InstalledApp, UpdateInfo } from "../../api/types";
import { appKey } from "../../utils/appkey";

defineProps<{ apps: InstalledApp[]; selected: string | null; updates: Record<string, UpdateInfo> }>();
defineEmits<{ select: [name: string] }>();
</script>

<style scoped>
.list {
  overflow: auto;
  height: 100%;
  border-right: 1px solid var(--border);
}
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  cursor: pointer;
}
.row:hover {
  background: var(--bg-hover);
}
.row.active {
  background: var(--bg-active);
}
.name {
  font-size: 14px;
}
.meta {
  font-size: 12px;
  color: var(--text-dim);
}
.hold {
  margin-left: 6px;
  font-size: 11px;
  color: #f0a020;
  border: 1px solid #f0a020;
  border-radius: 3px;
  padding: 0 4px;
}
.update {
  margin-left: 6px;
  font-size: 11px;
  color: #2080f0;
  border: 1px solid #2080f0;
  border-radius: 3px;
  padding: 0 4px;
}
.empty {
  padding: 24px;
  color: var(--text-dim);
  text-align: center;
}
</style>
