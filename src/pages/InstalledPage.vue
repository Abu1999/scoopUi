<template>
  <div class="page">
    <p>已安装 {{ apps.apps.length }} 个应用</p>
  </div>
</template>

<script setup lang="ts">
import { useAppsStore } from "../stores/apps";

const apps = useAppsStore();
</script>

<style scoped>
.page {
  padding: 16px;
  height: 100%;
  overflow: auto;
}
</style>
