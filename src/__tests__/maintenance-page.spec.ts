import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { NPopconfirm } from "naive-ui";

const messageError = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  runMaintenance: vi.fn().mockResolvedValue(undefined),
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

import MaintenancePage from "../pages/MaintenancePage.vue";
import { useLogsStore } from "../stores/logs";

function clickButton(wrapper: ReturnType<typeof mount>, text: string) {
  const button = wrapper.findAll("button").find((b) => b.text().includes(text));
  expect(button).toBeTruthy();
  return button!;
}

describe("MaintenancePage", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.runMaintenance.mockResolvedValue(undefined);
  });

  it("直接操作按钮调用对应 op", async () => {
    const wrapper = mount(MaintenancePage);
    await clickButton(wrapper, "运行检查").trigger("click");
    await flushPromises();
    expect(mocks.runMaintenance).toHaveBeenCalledWith("checkup", expect.any(String));
  });

  it("需确认的操作在确认后调用", async () => {
    const wrapper = mount(MaintenancePage);
    const confirms = wrapper.findAllComponents(NPopconfirm);
    expect(confirms.length).toBeGreaterThanOrEqual(4);
    await confirms[0].vm.$emit("positive-click");
    await flushPromises();
    expect(mocks.runMaintenance).toHaveBeenCalledWith("cache_rm_all", expect.any(String));
  });

  it("失败提示错误", async () => {
    const wrapper = mount(MaintenancePage);
    await clickButton(wrapper, "运行检查").trigger("click");
    await flushPromises();
    const taskId = mocks.runMaintenance.mock.calls[0][1];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalledWith("操作失败（退出码 1），详见任务日志");
  });

  it("任务运行中禁用按钮", () => {
    useLogsStore().start("t1", "维护");
    const wrapper = mount(MaintenancePage);
    expect(clickButton(wrapper, "运行检查").attributes("disabled")).toBeDefined();
  });
});
