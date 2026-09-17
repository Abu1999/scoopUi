<template>
  <div class="page">
    <n-table :bordered="false" size="small">
      <thead>
        <tr>
          <th>名称</th>
          <th>来源</th>
          <th>清单数</th>
          <th>类型</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="b in store.buckets" :key="b.name">
          <td>{{ b.name }}</td>
          <td class="source">{{ b.source ?? "（本地目录，非 git）" }}</td>
          <td>{{ b.manifest_count }}</td>
          <td>{{ b.is_git ? "git" : "目录" }}</td>
        </tr>
      </tbody>
    </n-table>
    <div v-if="store.error" class="error">{{ store.error }}</div>
  </div>
</template>

<script setup lang="ts">
import { NTable } from "naive-ui";
import { useBucketsStore } from "../stores/buckets";

const store = useBucketsStore();
</script>

<style scoped>
.page {
  padding: 16px;
  overflow: auto;
  height: 100%;
}
.source {
  font-family: Consolas, monospace;
  font-size: 12px;
  word-break: break-all;
}
.error {
  color: #d03050;
  margin-top: 12px;
}
</style>
