<template>
  <div class="section">
    <h3>下载加速（URL_PROXY）</h3>
    <n-descriptions :column="1" size="small" bordered>
      <n-descriptions-item label="当前代理前缀">
        {{ info.url_proxy ?? `默认（${info.default_url_proxy}）` }}
      </n-descriptions-item>
    </n-descriptions>
    <div class="row">
      <n-input v-model:value="proxyInput" size="small" class="input" placeholder="留空使用默认" />
      <n-button size="small" @click="saveProxy">保存代理前缀</n-button>
      <n-button size="small" @click="resetProxy">恢复默认</n-button>
      <n-button size="small" :loading="testing" @click="test">连通测试</n-button>
    </div>
    <n-alert v-if="testResult" :type="exitAlertType" size="small" class="alert">
      <div>出口 IP：{{ testResult.exit_info ?? "未获取到" }}（{{ exitHint }}）</div>
      <div>
        代理前缀{{ testResult.proxy_reachable ? "可用" : "不可用"
        }}{{ testResult.proxy_reachable && testResult.proxy_info ? `（${testResult.proxy_info}）` : "" }}
      </div>
    </n-alert>

    <h3>网络代理（proxy，host:port）</h3>
    <div class="row">
      <n-input v-model:value="plainProxyInput" size="small" class="input" placeholder="例如 127.0.0.1:7890，留空清除" />
      <n-button size="small" @click="savePlainProxy">保存</n-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NAlert, NButton, NDescriptions, NDescriptionsItem, NInput, useMessage } from "naive-ui";
import { setProxy, setUrlProxy, testUrlProxy } from "../../api/commands";
import type { MirrorInfo, ProxyTest } from "../../api/types";

const props = defineProps<{ info: MirrorInfo }>();
const emit = defineEmits<{ changed: [] }>();
const message = useMessage();
const proxyInput = ref(props.info.url_proxy ?? "");
const plainProxyInput = ref(props.info.proxy ?? "");
const testing = ref(false);
const testResult = ref<ProxyTest | null>(null);
const exitAlertType = computed(() => {
  if (testResult.value?.in_china === true) return "warning";
  if (testResult.value?.in_china === false) return "success";
  return "default";
});
const exitHint = computed(() => {
  if (testResult.value?.in_china === true) return "位于中国，建议启用代理";
  if (testResult.value?.in_china === false) return "不在中国，可直连";
  return "未获取到出口信息，将按默认策略处理";
});

watch(
  () => props.info,
  (value) => {
    proxyInput.value = value.url_proxy ?? "";
    plainProxyInput.value = value.proxy ?? "";
  },
);

async function saveProxy() {
  try {
    await setUrlProxy(proxyInput.value.trim() === "" ? null : proxyInput.value);
    message.success("已保存 URL_PROXY");
    emit("changed");
  } catch (e) {
    message.error(String(e));
  }
}

async function resetProxy() {
  try {
    await setUrlProxy(null);
    proxyInput.value = "";
    message.success("已恢复默认代理前缀");
    emit("changed");
  } catch (e) {
    message.error(String(e));
  }
}

async function savePlainProxy() {
  try {
    await setProxy(plainProxyInput.value.trim() === "" ? null : plainProxyInput.value);
    message.success("已保存代理设置");
    emit("changed");
  } catch (e) {
    message.error(String(e));
  }
}

async function test() {
  testing.value = true;
  try {
    testResult.value = await testUrlProxy(proxyInput.value.trim() === "" ? null : proxyInput.value);
  } catch (e) {
    message.error(String(e));
  } finally {
    testing.value = false;
  }
}
</script>

<style scoped>
.section {
  margin-top: 18px;
}
h3 {
  margin: 18px 0 8px;
  font-size: 14px;
}
h3:first-child {
  margin-top: 0;
}
.row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
}
.input {
  width: 280px;
}
.alert {
  margin-top: 10px;
}
</style>
