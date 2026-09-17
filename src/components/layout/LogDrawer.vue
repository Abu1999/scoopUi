<template>
  <div v-if="ui.logDrawerOpen" class="drawer">
    <div class="header">
      <span class="title">任务日志</span>
      <span v-if="runningCount > 0" class="running">{{ runningCount }} 个任务运行中</span>
      <div class="spacer" />
      <n-button v-if="lastRunningTask" size="tiny" type="error" @click="cancelLast">取消任务</n-button>
      <n-button size="tiny" @click="logs.clear()">清空</n-button>
      <n-button size="tiny" @click="ui.toggleLogs()">收起</n-button>
    </div>
    <div ref="bodyEl" class="body">
      <div v-if="logs.lines.length === 0" class="empty">暂无日志</div>
      <div v-for="(l, i) in logs.lines" :key="i" class="line" :class="l.stream">
        {{ l.line }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { NButton, useMessage } from "naive-ui";
import { useLogsStore } from "../../stores/logs";
import { useUiStore } from "../../stores/ui";
import { cancelTask } from "../../api/commands";

const logs = useLogsStore();
const ui = useUiStore();
const message = useMessage();
const bodyEl = ref<HTMLElement | null>(null);

const runningCount = computed(() => Object.values(logs.running).filter(Boolean).length);
const lastRunningTask = computed(() => Object.keys(logs.running).find((k) => logs.running[k]) ?? null);

watch(
  [() => logs.lines.length, () => ui.logDrawerOpen],
  async () => {
    await nextTick();
    if (bodyEl.value) bodyEl.value.scrollTop = bodyEl.value.scrollHeight;
  },
);

async function cancelLast() {
  const taskId = lastRunningTask.value;
  if (!taskId) return;
  try {
    const ok = await cancelTask(taskId);
    if (!ok) message.warning("任务已结束或不存在");
  } catch (e) {
    message.error(String(e));
  }
}
</script>

<style scoped>
.drawer {
  height: 200px;
  border-top: 1px solid #efeff5;
  display: flex;
  flex-direction: column;
  background: #fafafc;
}
.header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-bottom: 1px solid #efeff5;
}
.title {
  font-weight: 600;
  font-size: 13px;
}
.running {
  color: #2080f0;
  font-size: 12px;
}
.spacer {
  flex: 1;
}
.body {
  flex: 1;
  overflow: auto;
  padding: 8px 10px;
  font-family: Consolas, monospace;
  font-size: 12px;
  line-height: 1.6;
}
.line.stderr {
  color: #d03050;
}
.line.meta {
  color: #18a058;
}
.empty {
  color: #999;
}
</style>
