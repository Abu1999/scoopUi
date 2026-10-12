<template>
  <n-config-provider
    :theme="appearance.effectiveTheme === 'dark' ? darkTheme : null"
    :theme-overrides="themeOverrides"
    :locale="zhCN"
    :date-locale="dateZhCN"
  >
    <n-message-provider>
      <AppShell />
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { onMounted, watchEffect } from "vue";
import { NConfigProvider, NMessageProvider, darkTheme, dateZhCN, zhCN } from "naive-ui";
import AppShell from "./components/layout/AppShell.vue";
import { useAppearanceStore } from "./stores/appearance";

const appearance = useAppearanceStore();

const themeOverrides = {
  common: { primaryColor: "#2080f0", primaryColorHover: "#4098fc" },
};

watchEffect(() => {
  document.documentElement.classList.toggle("dark", appearance.effectiveTheme === "dark");
});

onMounted(() => {
  appearance.load();
});
</script>
