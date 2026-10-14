import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const messageError = vi.hoisted(() => vi.fn());
const messageSuccess = vi.hoisted(() => vi.fn());
const dialogOpen = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  setBucketSource: vi.fn().mockResolvedValue("https://github.com/ScoopInstaller/Main"),
  setScoopSource: vi.fn().mockResolvedValue("https://github.com/ScoopInstaller/Scoop"),
  setUrlProxy: vi.fn().mockResolvedValue(undefined),
  setProxy: vi.fn().mockResolvedValue(undefined),
  setAria2: vi.fn().mockResolvedValue(undefined),
  testUrlProxy: vi.fn(),
  installPackage: vi.fn().mockResolvedValue(undefined),
  getScoopEnv: vi.fn(),
  adoptScoopDir: vi.fn(),
  moveScoopDir: vi.fn(),
  removeScoopEnv: vi.fn(),
  getScoopConfig: vi.fn(),
  getMirrorInfo: vi.fn(),
  runScoop: vi.fn(),
  listInstalledApps: vi.fn().mockResolvedValue([]),
  listBuckets: vi.fn().mockResolvedValue([]),
  listUpdates: vi.fn().mockResolvedValue([]),
}));
vi.mock("../api/commands", () => mocks);
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: dialogOpen }));

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: messageError,
      success: messageSuccess,
      info: vi.fn(),
      warning: vi.fn(),
    }),
  };
});

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

import { NPopconfirm } from "naive-ui";
import AccelerationSection from "../components/settings/AccelerationSection.vue";
import Aria2Section from "../components/settings/Aria2Section.vue";
import MirrorSection from "../components/settings/MirrorSection.vue";
import SettingsPage from "../pages/SettingsPage.vue";
import { useLogsStore } from "../stores/logs";
import { useRootsStore } from "../stores/roots";
import type { MirrorInfo, ScoopEnvInfo } from "../api/types";

function info(extra: Partial<MirrorInfo> = {}): MirrorInfo {
  return {
    scoop_repo: "https://gitee.com/scoop-installer/scoop",
    scoop_remote: "https://gitee.com/scoop-installer/scoop.git",
    presets: {
      official: "https://github.com/ScoopInstaller/scoop",
      gitee: "https://gitee.com/scoop-installer/scoop",
      proxy: "https://scoop.201704.xyz/https://github.com/ScoopInstaller/scoop",
    },
    url_proxy: null,
    default_url_proxy: "https://scoop.201704.xyz",
    proxy: null,
    buckets: [
      {
        name: "main",
        remote: "https://gitee.com/scoop-installer/Main",
        presets: {
          official: "https://github.com/ScoopInstaller/Main",
          gitee: "https://gitee.com/scoop-installer/Main",
          proxy: "https://scoop.201704.xyz/https://github.com/ScoopInstaller/Main",
        },
      },
      { name: "custom", remote: "https://gitlab.com/x/y", presets: null },
    ],
    aria2_installed: false,
    aria2_enabled: true,
    aria2_split: null,
    aria2_max_connection: null,
    ...extra,
  };
}

describe("设置区块", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.testUrlProxy.mockResolvedValue({
      exit_info: "中国 广东 深圳 电信",
      in_china: true,
      proxy_reachable: true,
      proxy_info: "中国 广东",
    });
  });

  it("镜像区块切换软件源并通知刷新", async () => {
    const wrapper = mount(MirrorSection, { props: { info: info() } });
    const rows = wrapper.findAll(".bucket-row");
    expect(rows.length).toBe(2);
    const select = rows[0].findComponent({ name: "Select" });
    select.vm.$emit("update:value", "official");
    await flushPromises();
    expect(mocks.setBucketSource).toHaveBeenCalledWith("main", "official");
    expect(wrapper.emitted("changed")).toBeTruthy();
    expect(wrapper.text()).toContain("不支持一键切换");
  });

  it("镜像区块切换 scoop 自身源", async () => {
    const wrapper = mount(MirrorSection, { props: { info: info() } });
    const select = wrapper.findComponent({ name: "Select" });
    select.vm.$emit("update:value", "gitee");
    await flushPromises();
    expect(mocks.setScoopSource).toHaveBeenCalledWith("gitee");
  });

  it("下拉框显示当前源", () => {
    const wrapper = mount(MirrorSection, { props: { info: info() } });
    const scoopSelect = wrapper.findComponent({ name: "Select" });
    expect(scoopSelect.props("value")).toBe("gitee");
    const rows = wrapper.findAll(".bucket-row");
    expect(rows[0].findComponent({ name: "Select" }).props("value")).toBe("gitee");
  });

  it("加速区块保存自定义 URL_PROXY 与恢复默认", async () => {
    const wrapper = mount(AccelerationSection, { props: { info: info() } });
    await wrapper.find('input[placeholder="留空使用默认"]').setValue("https://my.proxy");
    const save = wrapper.findAll("button").find((b) => b.text().includes("保存代理前缀"));
    await save!.trigger("click");
    await flushPromises();
    expect(mocks.setUrlProxy).toHaveBeenCalledWith("https://my.proxy");

    const reset = wrapper.findAll("button").find((b) => b.text().includes("恢复默认"));
    await reset!.trigger("click");
    await flushPromises();
    expect(mocks.setUrlProxy).toHaveBeenLastCalledWith(null);
  });

  it("加速区块连通测试展示出口信息", async () => {
    const wrapper = mount(AccelerationSection, { props: { info: info() } });
    const button = wrapper.findAll("button").find((b) => b.text().includes("连通测试"));
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.testUrlProxy).toHaveBeenCalledWith(null);
    expect(wrapper.text()).toContain("中国 广东 深圳 电信");
    expect(wrapper.text()).toContain("代理前缀可用");
  });

  it("aria2 区块未安装时提供安装按钮并保存参数", async () => {
    const wrapper = mount(Aria2Section, { props: { info: info() } });
    expect(wrapper.text()).toContain("未安装");
    const install = wrapper.findAll("button").find((b) => b.text().includes("安装 aria2"));
    await install!.trigger("click");
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalledWith("aria2", null, null, expect.any(String));
    const taskId = mocks.installPackage.mock.calls[0][3];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(wrapper.emitted("changed")).toBeTruthy();

    const save = wrapper.findAll("button").find((b) => b.text().includes("保存 aria2 设置"));
    await save!.trigger("click");
    await flushPromises();
    expect(mocks.setAria2).toHaveBeenCalledWith(true, null, null);
  });

  it("任务运行时操作按钮禁用", () => {
    useLogsStore().start("t1", "安装");
    const wrapper = mount(AccelerationSection, { props: { info: info() } });
    const save = wrapper.findAll("button").find((b) => b.text().includes("保存代理前缀"));
    expect(save!.attributes("disabled")).toBeDefined();
  });

  it("加速区块出口未知时不误报可直连", async () => {
    mocks.testUrlProxy.mockResolvedValue({
      exit_info: null,
      in_china: null,
      proxy_reachable: false,
      proxy_info: null,
    });
    const wrapper = mount(AccelerationSection, { props: { info: info() } });
    const button = wrapper.findAll("button").find((b) => b.text().includes("连通测试"));
    await button!.trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("未获取到出口信息");
    expect(wrapper.text()).not.toContain("可直连");
  });

  it("镜像区块切换失败时提示错误且不通知刷新", async () => {
    mocks.setBucketSource.mockRejectedValueOnce("该仓库不是官方 GitHub/gitee 已知仓库，无法一键切换源");
    const wrapper = mount(MirrorSection, { props: { info: info() } });
    const rows = wrapper.findAll(".bucket-row");
    rows[0].findComponent({ name: "Select" }).vm.$emit("update:value", "official");
    await flushPromises();
    expect(messageError).toHaveBeenCalled();
    expect(wrapper.emitted("changed")).toBeFalsy();
  });

  it("scoop 源切换失败时提示错误且不通知刷新", async () => {
    mocks.setScoopSource.mockRejectedValueOnce("切换源失败: git 执行失败");
    const wrapper = mount(MirrorSection, { props: { info: info() } });
    wrapper.findComponent({ name: "Select" }).vm.$emit("update:value", "official");
    await flushPromises();
    expect(messageError).toHaveBeenCalled();
    expect(wrapper.emitted("changed")).toBeFalsy();
  });

  it("连通测试失败时复位 loading 并提示错误", async () => {
    mocks.testUrlProxy.mockRejectedValueOnce("连通测试失败");
    const wrapper = mount(AccelerationSection, { props: { info: info() } });
    const button = wrapper.findAllComponents({ name: "Button" }).find((b) => b.text().includes("连通测试"));
    await button!.trigger("click");
    await flushPromises();
    expect(button!.props("loading")).toBe(false);
    expect(messageError).toHaveBeenCalled();
  });

  it("aria2 安装失败时提示错误且不通知刷新", async () => {
    const wrapper = mount(Aria2Section, { props: { info: info() } });
    const install = wrapper.findAll("button").find((b) => b.text().includes("安装 aria2"));
    await install!.trigger("click");
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][3];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalled();
    expect(wrapper.emitted("changed")).toBeFalsy();
  });
});

const envFixture: ScoopEnvInfo = {
  path: "C:\\scoop",
  valid: true,
  app_count: 9,
  bucket_count: 4,
  has_portable_config: false,
};

describe("运行环境区块", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("展示安装目录与应用/软件源数量", async () => {
    const roots = useRootsStore();
    roots.env = envFixture;
    mocks.getScoopConfig.mockResolvedValue(null);
    mocks.getMirrorInfo.mockResolvedValue(null);
    const wrapper = mount(SettingsPage);
    await flushPromises();
    expect(wrapper.text()).toContain("C:\\scoop");
    expect(wrapper.text()).toContain("9 / 4");
    expect(wrapper.text()).not.toContain("全局目录");
  });

  it("修改位置迁移成功后刷新应用与软件源", async () => {
    const roots = useRootsStore();
    roots.env = envFixture;
    mocks.getScoopConfig.mockResolvedValue(null);
    mocks.getMirrorInfo.mockResolvedValue(null);
    mocks.moveScoopDir.mockResolvedValue({ ...envFixture, path: "D:\\scoop" });
    dialogOpen.mockResolvedValue("D:\\scoop");
    const wrapper = mount(SettingsPage);
    await flushPromises();
    const button = wrapper.findAll("button").find((b) => b.text().includes("修改位置"));
    await button!.trigger("click");
    await flushPromises();
    expect(dialogOpen).toHaveBeenCalled();
    expect(mocks.moveScoopDir).toHaveBeenCalledWith("D:\\scoop");
    expect(mocks.listInstalledApps).toHaveBeenCalled();
    expect(mocks.listBuckets).toHaveBeenCalled();
    expect(roots.env?.path).toBe("D:\\scoop");
    expect(messageSuccess).toHaveBeenCalled();
  });

  it("删除该环境后清空 store", async () => {
    const roots = useRootsStore();
    roots.env = envFixture;
    mocks.getScoopConfig.mockResolvedValue(null);
    mocks.getMirrorInfo.mockResolvedValue(null);
    mocks.removeScoopEnv.mockResolvedValue("C:\\scoop");
    const wrapper = mount(SettingsPage);
    await flushPromises();
    await wrapper.findComponent(NPopconfirm).vm.$emit("positive-click");
    await flushPromises();
    expect(mocks.removeScoopEnv).toHaveBeenCalled();
    expect(roots.env).toBeNull();
    expect(messageSuccess).toHaveBeenCalled();
  });
});
