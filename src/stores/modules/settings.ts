import { defineStore } from "pinia";
import request from "@/utils/request";

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    activeStageId: Number(localStorage.getItem("ll_active_stage_id") || 0),
    layout: {
      sidebarCollapsed: localStorage.getItem("ll_sidebar_collapsed") === "1",
    },
  }),
  actions: {
    async fetchSettings() {
      try {
        const settings = (await request({
          url: "/api/users/settings",
          method: "get",
        })) as any;
        const resolved = settings?.settings || settings?.data || settings || {};
        if (resolved) {
          if (resolved.active_stage_id) {
            this.activeStageId = resolved.active_stage_id;
            localStorage.setItem(
              "ll_active_stage_id",
              String(resolved.active_stage_id),
            );
            localStorage.setItem(
              "active_stage_id",
              String(resolved.active_stage_id),
            );
          }
        }
      } catch (error) {
        console.error("获取用户设置失败:", error);
      }
    },
    async saveSettings() {
      try {
        await request({
          url: "/api/users/settings",
          method: "post",
          data: {
            active_stage_id: this.activeStageId,
          },
        });
      } catch (error) {
        console.error("保存用户设置失败:", error);
        throw error;
      }
    },
    setActiveStage(stageId: number) {
      this.activeStageId = stageId;
      localStorage.setItem("ll_active_stage_id", String(stageId || 0));
      localStorage.setItem("active_stage_id", String(stageId || 0));
      void this.saveSettings();
    },
    setSidebarCollapsed(collapsed: boolean) {
      this.layout.sidebarCollapsed = collapsed;
      localStorage.setItem("ll_sidebar_collapsed", collapsed ? "1" : "0");
    },
  },
});
