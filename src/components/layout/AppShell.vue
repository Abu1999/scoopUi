<template>
  <div class="shell">
    <SideBar />
    <div class="main">
      <TopBar />
      <div class="content">
        <InstalledPage v-if="ui.activePage === 'installed'" />
        <BucketsPage v-else-if="ui.activePage === 'buckets'" />
        <SettingsPage v-else-if="ui.activePage === 'settings'" />
        <PlaceholderPage v-else :title="placeholderTitle" />
      </div>
      <LogDrawer />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from "vue";
import SideBar from "./SideBar.vue";
import TopBar from "./TopBar.vue";
import LogDrawer from "./LogDrawer.vue";
import InstalledPage from "../../pages/InstalledPage.vue";
import BucketsPage from "../../pages/BucketsPage.vue";
import SettingsPage from "../../pages/SettingsPage.vue";
import PlaceholderPage from "../../pages/PlaceholderPage.vue";
import { useUiStore } from "../../stores/ui";
import { useLogsStore } from "../../stores/logs";
import { useRootsStore } from "../../stores/roots";
import { useAppsStore } from "../../stores/apps";
import { useBucketsStore } from "../../stores/buckets";

const ui = useUiStore();
const placeholders: Record<string, string> = {
  store: "商店（M2：清单索引与搜索）",
  maintenance: "维护（M3：缓存清理与维护操作）",
  migrate: "环境迁移（M6：注册表扫描与接管）",
  profiles: "配置集（M5：Profile 切换）",
  backup: "备份（M5：快照与恢复）",
};
const placeholderTitle = computed(() => placeholders[ui.activePage] ?? "开发中");

onMounted(async () => {
  await useLogsStore()
    .init()
    .catch(() => {});
  await useRootsStore().load();
  await useAppsStore().load();
  await useBucketsStore().load();
});
</script>

<style scoped>
.shell {
  display: flex;
  height: 100vh;
  overflow: hidden;
  font-family: system-ui, "Microsoft YaHei", sans-serif;
}
.main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.content {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
</style>
