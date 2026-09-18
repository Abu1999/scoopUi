<template>
  <div class="page">
    <div class="side">
      <n-alert v-if="apps.error" type="error" size="small" class="alert">{{ apps.error }}</n-alert>
      <AppList :apps="apps.apps" :selected="apps.selectedName" class="list" @select="apps.select" />
    </div>
    <AppDetail :app="apps.selected" class="detail" />
  </div>
</template>

<script setup lang="ts">
import { NAlert } from "naive-ui";
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
.side {
  width: 340px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.alert {
  margin: 8px 8px 0;
}
.list {
  flex: 1;
  min-height: 0;
}
.detail {
  flex: 1;
  min-width: 0;
}
</style>
