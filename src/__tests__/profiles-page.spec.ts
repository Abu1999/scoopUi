import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const messageError = vi.hoisted(() => vi.fn());
const messageSuccess = vi.hoisted(() => vi.fn());
const dialogOpen = vi.hoisted(() => vi.fn());
const dialogSave = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  listProfiles: vi.fn(),
  getProfile: vi.fn(),
  profileDiff: vi.fn(),
  saveProfileFromCurrent: vi.fn(),
  importProfile: vi.fn(),
  exportProfile: vi.fn(),
  renameProfile: vi.fn(),
  deleteProfile: vi.fn(),
  applyProfileSettings: vi.fn(),
  bucketAdd: vi.fn(),
  installPackage: vi.fn(),
  uninstallPackage: vi.fn(),
  listInstalledApps: vi.fn(),
  listBuckets: vi.fn(),
  listUpdates: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: dialogOpen, save: dialogSave }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
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

import ProfilesPage from "../pages/ProfilesPage.vue";
import { useLogsStore } from "../stores/logs";
import { useProfilesStore } from "../stores/profiles";
import type { Profile, ProfileDiff } from "../api/types";

const summary = { name: "工作", created: 1758614400, bucket_count: 1, app_count: 1, settings_count: 1 };

const profileFixture: Profile = {
  version: 1,
  name: "工作",
  created: 1758614400,
  buckets: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
  apps: [{ name: "7zip", bucket: "main" }],
  settings: { url_proxy: "https://scoop.201704.xyz" },
};

const diffFixture: ProfileDiff = {
  missing_buckets: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
  extra_buckets: [],
  bucket_source_conflicts: [],
  missing_apps: [{ name: "7zip", bucket: "main" }],
  extra_apps: [],
  settings: [{ key: "url_proxy", current: null, target: "https://scoop.201704.xyz" }],
};

async function mountLoaded() {
  const pinia = createPinia();
  setActivePinia(pinia);
  await useProfilesStore().load();
  const wrapper = mount(ProfilesPage, { global: { plugins: [pinia] }, attachTo: document.body });
  await flushPromises();
  return wrapper;
}

function bodyButton(text: string): HTMLButtonElement {
  const found = Array.from(document.body.querySelectorAll("button")).find((b) =>
    b.textContent?.includes(text),
  );
  expect(found).toBeTruthy();
  return found as HTMLButtonElement;
}

describe("ProfilesPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([]);
    mocks.listBuckets.mockResolvedValue([]);
    mocks.listUpdates.mockResolvedValue([]);
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("渲染列表、详情与差异", async () => {
    mocks.listProfiles.mockResolvedValue([summary]);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue(diffFixture);
    const wrapper = await mountLoaded();
    expect(wrapper.text()).toContain("工作");
    expect(wrapper.text()).toContain("缺少软件源");
    expect(wrapper.text()).toContain("extras");
    expect(wrapper.text()).toContain("缺少应用");
    expect(wrapper.text()).toContain("7zip");
    expect(wrapper.text()).toContain("url_proxy");
  });

  it("导入时先选文件再确认名称", async () => {
    mocks.listProfiles.mockResolvedValue([]);
    dialogOpen.mockResolvedValue("C:\\tmp\\scoopfile.json");
    mocks.importProfile.mockResolvedValue(profileFixture);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue(diffFixture);
    const wrapper = await mountLoaded();
    const importBtn = wrapper.findAll("button").find((b) => b.text().includes("导入"));
    await importBtn!.trigger("click");
    await flushPromises();
    const input = document.body.querySelector("input") as HTMLInputElement;
    expect(input.value).toBe("scoopfile");
    bodyButton("确认导入").click();
    await flushPromises();
    expect(mocks.importProfile).toHaveBeenCalledWith("C:\\tmp\\scoopfile.json", "scoopfile");
  });

  it("导出调用保存对话框与命令", async () => {
    mocks.listProfiles.mockResolvedValue([summary]);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue(diffFixture);
    dialogSave.mockResolvedValue("C:\\out\\工作.json");
    mocks.exportProfile.mockResolvedValue(undefined);
    const wrapper = await mountLoaded();
    const exportBtn = wrapper.findAll("button").find((b) => b.text().includes("导出"));
    await exportBtn!.trigger("click");
    await flushPromises();
    expect(mocks.exportProfile).toHaveBeenCalledWith("工作", "C:\\out\\工作.json");
  });

  it("应用失败时提示错误并保留步骤状态", async () => {
    mocks.listProfiles.mockResolvedValue([summary]);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue({
      ...diffFixture,
      missing_buckets: [],
      settings: [],
  missing_apps: [{ name: "7zip", bucket: "main" }],
    });
    mocks.installPackage.mockResolvedValue(undefined);
    const wrapper = await mountLoaded();
    const applyBtn = wrapper.findAll("button").find((b) => b.text() === "应用");
    await applyBtn!.trigger("click");
    await flushPromises();
    bodyButton("开始执行").click();
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][3];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalled();
    expect(document.body.textContent).toContain("失败");
  });

  it("应用命令拒绝时提示错误且不抛出", async () => {
    mocks.listProfiles.mockResolvedValue([summary]);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue(diffFixture);
    const wrapper = await mountLoaded();
    const applyBtn = wrapper.findAll("button").find((b) => b.text() === "应用");
    await applyBtn!.trigger("click");
    await flushPromises();
    mocks.profileDiff.mockRejectedValueOnce("差异计算失败");
    bodyButton("开始执行").click();
    await flushPromises();
    expect(messageError).toHaveBeenCalledWith("差异计算失败");
  });

  it("首次挂载自动加载配置集列表", async () => {
    mocks.listProfiles.mockResolvedValue([summary]);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue(diffFixture);
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mount(ProfilesPage, { global: { plugins: [pinia] }, attachTo: document.body });
    await flushPromises();
    expect(mocks.listProfiles).toHaveBeenCalled();
    expect(wrapper.text()).toContain("工作");
  });

  it("差异不可用时仍展示详情", async () => {
    mocks.listProfiles.mockResolvedValue([summary]);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockRejectedValue("未找到可用的 scoop 根目录");
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mount(ProfilesPage, { global: { plugins: [pinia] }, attachTo: document.body });
    await flushPromises();
    expect(wrapper.text()).toContain("工作");
    expect(wrapper.text()).toContain("差异暂不可用");
    expect(wrapper.text()).not.toContain("与当前状态一致");
  });
});
