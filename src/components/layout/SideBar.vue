<template>
  <div class="sidebar">
    <div class="brand">ScoopUI</div>
    <n-menu :value="ui.activePage" :options="options" @update:value="onSelect" />
  </div>
</template>

<script setup lang="ts">
import { NMenu } from "naive-ui";
import type { MenuOption } from "naive-ui";
import { useUiStore } from "../../stores/ui";
import type { PageId } from "../../stores/ui";

const ui = useUiStore();

const options: MenuOption[] = [
  { label: "已安装", key: "installed" },
  { label: "商店", key: "store" },
  { label: "桶管理", key: "buckets" },
  { label: "维护", key: "maintenance" },
  { label: "环境迁移", key: "migrate" },
  { label: "配置集", key: "profiles" },
  { label: "备份", key: "backup" },
  { label: "设置", key: "settings" },
];

function onSelect(key: string) {
  ui.setPage(key as PageId);
}
</script>

<style scoped>
.sidebar {
  width: 180px;
  border-right: 1px solid #efeff5;
  display: flex;
  flex-direction: column;
}
.brand {
  padding: 16px;
  font-weight: 600;
  font-size: 16px;
}
</style>
