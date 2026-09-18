<template>
  <img
    v-if="url"
    class="icon-img"
    :src="url"
    :alt="name"
    :style="{
      width: `${size}px`,
      height: `${size}px`,
      borderRadius: `${Math.round(size * 0.2)}px`,
    }"
  />
  <div
    v-else
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
import { computed, onMounted, watch } from "vue";
import { avatarColor, avatarLetter } from "../../utils/avatar";
import { useIconsStore } from "../../stores/icons";

const props = withDefaults(
  defineProps<{ name: string; size?: number; source?: string | null }>(),
  { size: 24, source: null },
);

const icons = useIconsStore();
const url = computed(() => (props.source ? icons.resolve(props.source) : null));

onMounted(() => {
  if (props.source) icons.ensure(props.source);
});

watch(
  () => props.source,
  (source) => {
    if (source) icons.ensure(source);
  },
);
</script>

<style scoped>
.icon,
.icon-img {
  flex-shrink: 0;
}
.icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  font-weight: 600;
}
.icon-img {
  display: inline-block;
  object-fit: contain;
}
</style>
