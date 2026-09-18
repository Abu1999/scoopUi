<template>
  <n-config-provider :theme-overrides="themeOverrides" :locale="zhCN" :date-locale="dateZhCN">
    <n-message-provider>
      <AppShell />
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { NConfigProvider, NMessageProvider, dateZhCN, zhCN } from "naive-ui";
import AppShell from "./components/layout/AppShell.vue";

const themeOverrides = {
  common: { primaryColor: "#2080f0", primaryColorHover: "#4098fc" },
};
</script>
