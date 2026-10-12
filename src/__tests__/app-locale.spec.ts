import { mount } from "@vue/test-utils";
import { createPinia } from "pinia";
import { NConfigProvider, dateZhCN, zhCN } from "naive-ui";
import { describe, expect, it } from "vitest";
import App from "../App.vue";

describe("App locale", () => {
  it("为 naive-ui 配置中文 locale", () => {
    const wrapper = mount(App, {
      global: { plugins: [createPinia()], stubs: { AppShell: true } },
    });
    const provider = wrapper.findComponent(NConfigProvider);
    expect(provider.props("locale")).toBe(zhCN);
    expect(provider.props("dateLocale")).toBe(dateZhCN);
  });
});
