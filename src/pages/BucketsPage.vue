<template>
  <div class="page">
    <p>共 {{ buckets.buckets.length }} 个桶</p>
    <div v-if="buckets.error" class="error">{{ buckets.error }}</div>
  </div>
</template>

<script setup lang="ts">
import { useBucketsStore } from "../stores/buckets";

const buckets = useBucketsStore();
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
