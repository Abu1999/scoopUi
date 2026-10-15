<template>
  <div class="section">
    <h3>aria2 下载加速</h3>
    <n-descriptions :column="1" size="small" bordered>
      <n-descriptions-item label="安装状态">
        {{ installed ? "已安装" : "未安装" }}
      </n-descriptions-item>
    </n-descriptions>
    <div class="row">
      <n-button v-if="!installed" size="small" @click="installAria2">安装 aria2</n-button>
      <template v-else>
        <n-checkbox v-model:checked="enabled">启用 aria2</n-checkbox>
        <n-input-number
          v-model:value="split"
          size="small"
          :min="1"
          :max="64"
          placeholder="5"
          class="num"
        />
        <span class="label">分片</span>
        <n-input-number
          v-model:value="maxConn"
          size="small"
          :min="1"
          :max="64"
          placeholder="5"
          class="num"
        />
        <span class="label">单服务器连接</span>
        <n-button size="small" @click="save">保存 aria2 设置</n-button>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import {
  NButton,
  NCheckbox,
  NDescriptions,
  NDescriptionsItem,
  NInputNumber,
  useMessage,
} from "naive-ui";
import { installPackage, setAria2 } from "../../api/commands";
import type { MirrorInfo } from "../../api/types";
import { useUiStore } from "../../stores/ui";
import { runTask } from "../../utils/tasks";

const props = defineProps<{ info: MirrorInfo }>();
const emit = defineEmits<{ changed: [] }>();
const message = useMessage();
const ui = useUiStore();
const installed = ref(props.info.aria2_installed);
const enabled = ref(props.info.aria2_enabled);
const split = ref<number | null>(props.info.aria2_split);
const maxConn = ref<number | null>(props.info.aria2_max_connection);

watch(
  () => props.info,
  (value) => {
    installed.value = value.aria2_installed;
    enabled.value = value.aria2_enabled;
    split.value = value.aria2_split;
    maxConn.value = value.aria2_max_connection;
  },
);

async function installAria2() {
  ui.logDrawerOpen = true;
  const record = await runTask("安装 aria2", (taskId) =>
    installPackage("aria2", null, null, taskId),
  );
  if (record.success) {
    installed.value = true;
    message.success("aria2 安装完成");
    emit("changed");
  } else {
    message.error(
      record.hint
        ? `安装失败（退出码 ${record.code ?? "未知"}）：${record.hint}`
        : `安装失败（退出码 ${record.code ?? "未知"}），详见任务日志`,
    );
  }
}

async function save() {
  try {
    await setAria2(enabled.value, split.value ?? null, maxConn.value ?? null);
    message.success("已保存 aria2 设置");
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
  flex-wrap: wrap;
}
.num {
  width: 90px;
}
.label {
  color: var(--text-dim);
  font-size: 12px;
}
</style>
