<template>
  <div
    class="icon"
    :style="{
      width: `${size}px`,
      height: `${size}px`,
      background: avatarColor(name),
      fontSize: `${Math.round(size * 0.45)}px`,
      borderRadius: `${Math.round(size * 0.2)}px`,
    }"
  >
    {{ avatarLetter(name) }}
  </div>
</template>

<script setup lang="ts">
import { avatarColor, avatarLetter } from "../../utils/avatar";

withDefaults(defineProps<{ name: string; size?: number }>(), { size: 24 });
</script>

<style scoped>
.icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  font-weight: 600;
  flex-shrink: 0;
}
</style>
