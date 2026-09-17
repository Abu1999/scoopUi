<template>
  <div class="list">
    <div v-if="apps.length === 0" class="empty">没有已安装的应用</div>
    <div
      v-for="app in apps"
      :key="app.name"
      class="row"
      :class="{ active: app.name === selected }"
      @click="$emit('select', app.name)"
    >
      <AppIcon :name="app.name" :size="28" />
      <div class="info">
        <div class="name">
          {{ app.name }}
          <span v-if="app.hold" class="hold">锁定</span>
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
import type { InstalledApp } from "../../api/types";

defineProps<{ apps: InstalledApp[]; selected: string | null }>();
defineEmits<{ select: [name: string] }>();
</script>

<style scoped>
.list {
  overflow: auto;
  height: 100%;
  border-right: 1px solid #efeff5;
}
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  cursor: pointer;
}
.row:hover {
  background: #f5f7fa;
}
.row.active {
  background: #eaf3ff;
}
.name {
  font-size: 14px;
}
.meta {
  font-size: 12px;
  color: #888;
}
.hold {
  margin-left: 6px;
  font-size: 11px;
  color: #f0a020;
  border: 1px solid #f0a020;
  border-radius: 3px;
  padding: 0 4px;
}
.empty {
  padding: 24px;
  color: #999;
  text-align: center;
}
</style>
