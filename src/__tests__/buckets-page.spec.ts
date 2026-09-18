import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { NPopconfirm, NSelect } from "naive-ui";

const messageError = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  listBuckets: vi.fn(),
  listKnownBuckets: vi.fn().mockResolvedValue(["extras", "java", "games"]),
  detectRoots: vi.fn().mockResolvedValue([]),
  getActiveRoot: vi.fn().mockResolvedValue(null),
  bucketAdd: vi.fn().mockResolvedValue(undefined),
  bucketRemove: vi.fn().mockResolvedValue(undefined),
  updatePackage: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../api/commands", () => mocks);

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: messageError,
      success: vi.fn(),
      info: vi.fn(),
      warning: vi.fn(),
    }),
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import BucketsPage from "../pages/BucketsPage.vue";
import { useBucketsStore } from "../stores/buckets";
import { useLogsStore } from "../stores/logs";

describe("BucketsPage 操作", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.listBuckets.mockResolvedValue([
      { name: "main", source: "https://github.com/ScoopInstaller/Main", is_git: true, manifest_count: 1641 },
    ]);
  });

  it("添加桶调用 bucketAdd 并刷新列表", async () => {
    const wrapper = mount(BucketsPage);
    await flushPromises();
    expect(useBucketsStore().buckets).toHaveLength(1);
    await wrapper.findComponent(NSelect).vm.$emit("update:value", "java");
    await wrapper.find('input[placeholder="仓库地址（可选，已知桶可留空）"]').setValue("https://example.com/java.git");
    const button = wrapper.findAll("button").find((b) => b.text().includes("添加桶"));
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.bucketAdd).toHaveBeenCalledWith("java", "https://example.com/java.git", expect.any(String));
    const taskId = mocks.bucketAdd.mock.calls[0][2];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(mocks.listBuckets).toHaveBeenCalledTimes(2);
  });

  it("已知桶仓库留空时传 null", async () => {
    const wrapper = mount(BucketsPage);
    await flushPromises();
    await wrapper.findComponent(NSelect).vm.$emit("update:value", "extras");
    const button = wrapper.findAll("button").find((b) => b.text().includes("添加桶"));
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.bucketAdd).toHaveBeenCalledWith("extras", null, expect.any(String));
  });

  it("下拉展示已知桶并可直接选择", async () => {
    const wrapper = mount(BucketsPage);
    await flushPromises();
    const select = wrapper.findComponent(NSelect);
    expect(select.props("options")).toEqual([
      { label: "extras", value: "extras" },
      { label: "java", value: "java" },
      { label: "games", value: "games" },
    ]);
    select.vm.$emit("update:value", "games");
    await flushPromises();
    const button = wrapper.findAll("button").find((b) => b.text().includes("添加桶"));
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.bucketAdd).toHaveBeenCalledWith("games", null, expect.any(String));
  });

  it("任务运行中确认删除不会再次发起", async () => {
    const wrapper = mount(BucketsPage);
    await flushPromises();
    useLogsStore().start("t1", "更新");
    await wrapper.findComponent(NPopconfirm).vm.$emit("positive-click");
    await flushPromises();
    expect(mocks.bucketRemove).not.toHaveBeenCalled();
  });

  it("删除桶需确认且失败提示错误", async () => {
    const wrapper = mount(BucketsPage);
    await flushPromises();
    await wrapper.findComponent(NPopconfirm).vm.$emit("positive-click");
    await flushPromises();
    expect(mocks.bucketRemove).toHaveBeenCalledWith("main", expect.any(String));
    const taskId = mocks.bucketRemove.mock.calls[0][1];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalledWith("操作失败（退出码 1），详见任务日志");
    expect(mocks.listBuckets).toHaveBeenCalledTimes(1);
  });

  it("更新 Scoop 与桶调用 updatePackage(null,false,false)", async () => {
    const wrapper = mount(BucketsPage);
    await flushPromises();
    const button = wrapper.findAll("button").find((b) => b.text().includes("更新 Scoop 与桶"));
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.updatePackage).toHaveBeenCalledWith(null, false, false, expect.any(String));
  });

  it("有任务运行时按钮禁用", async () => {
    useLogsStore().start("t1", "测试任务");
    const wrapper = mount(BucketsPage);
    await flushPromises();
    const addButton = wrapper.findAll("button").find((b) => b.text().includes("添加桶"));
    expect(addButton!.attributes("disabled")).toBeDefined();
  });
});
