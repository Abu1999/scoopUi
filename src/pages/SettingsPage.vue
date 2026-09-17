<template>
  <div class="page">
    <p>当前根目录：{{ roots.active?.path ?? "未设置" }}</p>
    <div v-if="roots.error" class="error">{{ roots.error }}</div>
  </div>
</template>

<script setup lang="ts">
import { useRootsStore } from "../stores/roots";

const roots = useRootsStore();
</script>

<style scoped>
.page {
  padding: 16px;
  height: 100%;
  overflow: auto;
}
.error {
  color: #d03050;
  margin-top: 12px;
}
</style>
