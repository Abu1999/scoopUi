<template>
  <n-config-provider :theme-overrides="{ common: { primaryColor: '#2080f0' } }">
    <n-message-provider>
      <div style="padding: 24px; font-family: system-ui, sans-serif">
        <h2>ScoopUI</h2>
        <p>环境就绪，进入 M1 开发。</p>
      </div>
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { NConfigProvider, NMessageProvider } from "naive-ui";
</script>
