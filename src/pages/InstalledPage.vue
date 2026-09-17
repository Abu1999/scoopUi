<template>
  <div class="page">
    <AppList :apps="apps.apps" :selected="apps.selectedName" class="list" @select="apps.select" />
    <AppDetail :app="apps.selected" class="detail" />
  </div>
</template>

<script setup lang="ts">
import AppList from "../components/app/AppList.vue";
import AppDetail from "../components/app/AppDetail.vue";
import { useAppsStore } from "../stores/apps";

const apps = useAppsStore();
</script>

<style scoped>
.page {
  display: flex;
  height: 100%;
}
.list {
  width: 280px;
  flex-shrink: 0;
}
.detail {
  flex: 1;
  min-width: 0;
}
</style>
