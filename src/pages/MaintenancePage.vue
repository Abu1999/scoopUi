<template>
  <div class="page">
    <h3>维护操作</h3>
    <p class="tip">操作为真实执行：点击按钮并按提示确认后运行，进度与结果见底部任务日志抽屉（运行中可取消）。</p>
    <div class="grid">
      <div v-for="op in ops" :key="op.key" class="card">
        <div class="title">{{ op.title }}</div>
        <div class="desc">{{ op.desc }}</div>
        <code class="cmd">{{ op.command }}</code>
        <n-popconfirm
          v-if="op.confirm"
          :positive-button-props="{ disabled: busy }"
          @positive-click="run(op)"
        >
          <template #trigger>
            <n-button size="small" :disabled="busy">{{ op.button }}</n-button>
          </template>
          {{ op.confirm }}
        </n-popconfirm>
        <n-button v-else size="small" :disabled="busy" @click="run(op)">{{ op.button }}</n-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { NButton, NPopconfirm, useMessage } from "naive-ui";
import { runMaintenance } from "../api/commands";
import { useLogsStore } from "../stores/logs";
import { useUiStore } from "../stores/ui";
import { runTask } from "../utils/tasks";

interface MaintenanceOp {
  key: string;
  title: string;
  desc: string;
  command: string;
  button: string;
  confirm?: string;
}

const ops: MaintenanceOp[] = [
  {
    key: "cache_rm_all",
    title: "清理下载缓存",
    desc: "删除所有已下载的安装包缓存，下次安装需重新下载",
    command: "scoop cache rm -a",
    button: "清理缓存",
    confirm: "确定清空下载缓存（scoop cache rm -a）？",
  },
  {
    key: "cleanup_all",
    title: "清理旧版本（用户应用）",
    desc: "删除用户应用中保留的历史版本目录",
    command: "scoop cleanup -a",
    button: "清理旧版本",
    confirm: "确定清理用户应用的旧版本（scoop cleanup -a）？",
  },
  {
    key: "cleanup_global",
    title: "清理旧版本（全局应用）",
    desc: "删除全局应用中保留的历史版本目录",
    command: "scoop cleanup -a -g",
    button: "清理全局旧版本",
    confirm: "确定清理全局应用的旧版本（scoop cleanup -a -g）？",
  },
  {
    key: "checkup",
    title: "检查问题",
    desc: "运行 scoop checkup 诊断环境与配置问题",
    command: "scoop checkup",
    button: "运行检查",
  },
  {
    key: "reset_all",
    title: "重置 shims",
    desc: "为全部应用重建 shim（用于解决命令冲突）",
    command: "scoop reset -a",
    button: "重置全部 shims",
    confirm: "确定重置全部应用的 shim（scoop reset -a）？",
  },
];

const logs = useLogsStore();
const ui = useUiStore();
const message = useMessage();
const busy = computed(() => logs.busy);

async function run(op: MaintenanceOp) {
  if (busy.value) return;
  ui.logDrawerOpen = true;
  const record = await runTask(op.title, (taskId) => runMaintenance(op.key, taskId));
  if (!record.success) {
    message.error(`操作失败（退出码 ${record.code ?? "未知"}），详见任务日志`);
  }
}
</script>

<style scoped>
.page {
  padding: 16px;
  overflow: auto;
  height: 100%;
}
h3 {
  margin: 0 0 8px;
  font-size: 14px;
}
.tip {
  color: #999;
  font-size: 12px;
  margin: 0 0 14px;
}
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 12px;
}
.card {
  border: 1px solid #efeff5;
  border-radius: 6px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  align-items: flex-start;
}
.title {
  font-weight: 600;
  font-size: 13px;
}
.desc {
  color: #888;
  font-size: 12px;
  flex: 1;
}
.cmd {
  font-family: Consolas, monospace;
  font-size: 12px;
  color: #555;
  background: #f7f7f9;
  border-radius: 3px;
  padding: 2px 6px;
}
</style>
