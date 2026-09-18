<template>
  <div class="page">
    <div class="main">
      <div class="toolbar">
        <n-input
          v-model:value="input"
          clearable
          placeholder="搜索应用（名称或描述）"
          class="search"
          @update:value="onInput"
        />
        <n-button-group class="view-toggle">
          <n-button size="small" :type="view === 'list' ? 'primary' : 'default'" @click="view = 'list'">
            列表
          </n-button>
          <n-button size="small" :type="view === 'grid' ? 'primary' : 'default'" @click="view = 'grid'">
            卡片
          </n-button>
        </n-button-group>
        <n-button size="small" :loading="pkgs.refreshing" @click="refresh">刷新索引</n-button>
        <span v-if="pkgs.indexCount !== null" class="count">共 {{ pkgs.indexCount }} 个包</span>
      </div>

      <n-alert v-if="pkgs.error" type="error" size="small" class="alert">{{ pkgs.error }}</n-alert>
      <div v-if="pkgs.results.length === 0 && !pkgs.error" class="hint">
        {{ pkgs.loading ? "搜索中…" : "没有匹配的应用" }}
      </div>
      <div v-else-if="pkgs.results.length > 0" class="results" :class="view" @scroll="onScroll">
        <template v-if="view === 'list'">
          <div
            v-for="p in pkgs.results"
            :key="`${p.bucket}/${p.name}`"
            class="row"
            :class="{ active: p.name === pkgs.selectedName }"
            @click="pkgs.select(p.name)"
          >
            <AppIcon :name="p.name" :size="28" :source="p.icon ?? p.homepage" />
            <div class="info">
              <div class="name">
                {{ p.name }}
                <n-tag v-if="p.deprecated" size="tiny" type="warning">已弃用</n-tag>
              </div>
              <div class="meta">{{ p.version }}<span v-if="p.bucket"> · {{ p.bucket }}</span></div>
            </div>
          </div>
        </template>
        <template v-else>
          <div
            v-for="p in pkgs.results"
            :key="`${p.bucket}/${p.name}`"
            class="card"
            :class="{ active: p.name === pkgs.selectedName }"
            @click="pkgs.select(p.name)"
          >
            <AppIcon :name="p.name" :size="48" :source="p.icon ?? p.homepage" />
            <div class="card-name">{{ p.name }}</div>
            <div class="card-meta">{{ p.version }}</div>
          </div>
        </template>
        <div v-if="pkgs.loading" class="load-state">加载中…</div>
        <div v-else-if="pkgs.loadedAll" class="load-state">已显示全部 {{ pkgs.results.length }} 条</div>
      </div>
    </div>
    <PackageDetail :pkg="pkgs.selected" class="detail" />
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { NAlert, NButton, NButtonGroup, NInput, NTag } from "naive-ui";
import AppIcon from "../components/app/AppIcon.vue";
import PackageDetail from "../components/store/PackageDetail.vue";
import { usePackagesStore } from "../stores/packages";
import { useRootsStore } from "../stores/roots";

const pkgs = usePackagesStore();
const roots = useRootsStore();
const input = ref(pkgs.query);
const view = ref<"list" | "grid">("list");
let timer: ReturnType<typeof setTimeout> | null = null;

function onInput(value: string) {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    pkgs.search(value);
  }, 300);
}

function refresh() {
  pkgs.refresh();
}

const LOAD_MORE_THRESHOLD = 200;

function onScroll(e: Event) {
  const el = e.target as HTMLElement;
  if (el.scrollHeight - el.scrollTop - el.clientHeight < LOAD_MORE_THRESHOLD) {
    pkgs.loadMore();
  }
}

watch(
  () => roots.active?.path,
  () => {
    pkgs.search(pkgs.query);
  },
);

onMounted(() => {
  pkgs.search(pkgs.query);
});

onBeforeUnmount(() => {
  if (timer) clearTimeout(timer);
});
</script>

<style scoped>
.page {
  display: flex;
  height: 100%;
}
.main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  border-bottom: 1px solid #efeff5;
}
.search {
  flex: 1;
  min-width: 0;
}
.count {
  color: #888;
  font-size: 12px;
  white-space: nowrap;
}
.alert {
  margin: 8px 8px 0;
}
.hint {
  padding: 32px;
  color: #999;
  text-align: center;
}
.results {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 8px;
}
.results.list {
  padding: 0;
}
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  cursor: pointer;
}
.row:hover {
  background: #f5f7fa;
}
.row.active {
  background: #eaf3ff;
}
.name {
  font-size: 14px;
}
.meta {
  font-size: 12px;
  color: #888;
}
.results.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
  gap: 8px;
  align-content: start;
}
.card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 12px 8px;
  border: 1px solid #efeff5;
  border-radius: 6px;
  cursor: pointer;
}
.card:hover {
  background: #f5f7fa;
}
.card.active {
  border-color: #2080f0;
  background: #eaf3ff;
}
.card-name {
  font-size: 13px;
  text-align: center;
  word-break: break-all;
}
.card-meta {
  font-size: 12px;
  color: #888;
}
.load-state {
  padding: 12px;
  text-align: center;
  color: #999;
  font-size: 12px;
  grid-column: 1 / -1;
}
.detail {
  width: 380px;
  flex-shrink: 0;
  border-left: 1px solid #efeff5;
}
</style>
