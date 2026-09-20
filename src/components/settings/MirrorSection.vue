<template>
  <div class="section">
    <h3>镜像源</h3>
    <n-descriptions :column="1" size="small" bordered>
      <n-descriptions-item label="scoop 仓库当前源">
        {{ info.scoop_remote ?? info.scoop_repo ?? "-" }}
      </n-descriptions-item>
    </n-descriptions>
    <div class="row">
      <n-select
        :value="scoopSource"
        size="small"
        class="source-select"
        :disabled="busy || !info.presets"
        :options="sourceOptions"
        placeholder="选择 scoop 仓库源"
        @update:value="applyScoop"
      />
      <span v-if="!info.presets" class="hint">当前仓库不是官方/gitee 已知仓库，不支持一键切换</span>
    </div>

    <n-table :bordered="false" size="small" class="table">
      <thead>
        <tr>
          <th>桶</th>
          <th>当前源</th>
          <th>切换</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="b in info.buckets" :key="b.name" class="bucket-row">
          <td>{{ b.name }}</td>
          <td class="source">{{ b.remote ?? "（非 git）" }}</td>
          <td>
            <template v-if="b.presets">
              <n-select
                :value="null"
                size="tiny"
                class="source-select"
                :disabled="busy"
                :options="sourceOptions"
                placeholder="选择"
                @update:value="(v: string) => applyBucket(b.name, v)"
              />
            </template>
            <span v-else class="hint">不支持一键切换</span>
          </td>
        </tr>
      </tbody>
    </n-table>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { NDescriptions, NDescriptionsItem, NSelect, NTable, useMessage } from "naive-ui";
import { setBucketSource, setScoopSource } from "../../api/commands";
import type { MirrorInfo } from "../../api/types";
import { useLogsStore } from "../../stores/logs";

const props = defineProps<{ info: MirrorInfo }>();
const emit = defineEmits<{ changed: [] }>();
const message = useMessage();
const logs = useLogsStore();
const busy = computed(() => logs.busy);
const scoopSource = ref<string | null>(null);
const sourceOptions = [
  { label: "官方 GitHub", value: "official" },
  { label: "gitee 镜像", value: "gitee" },
  { label: "201704 代理", value: "proxy" },
];

async function applyScoop(source: string) {
  if (busy.value || !props.info.presets) return;
  try {
    const url = await setScoopSource(source);
    scoopSource.value = source;
    message.success(`已切换 scoop 仓库源: ${url}`);
    emit("changed");
  } catch (e) {
    message.error(String(e));
  }
}

async function applyBucket(name: string, source: string) {
  if (busy.value) return;
  try {
    const url = await setBucketSource(name, source);
    message.success(`桶 ${name} 已切换: ${url}`);
    emit("changed");
  } catch (e) {
    message.error(String(e));
  }
}
</script>

<style scoped>
.section {
  margin-top: 18px;
}
h3 {
  margin: 0 0 8px;
  font-size: 14px;
}
.row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
}
.table {
  margin-top: 12px;
}
.source-select {
  width: 160px;
}
.source {
  font-family: Consolas, monospace;
  font-size: 12px;
  word-break: break-all;
}
.hint {
  color: #999;
  font-size: 12px;
}
</style>
