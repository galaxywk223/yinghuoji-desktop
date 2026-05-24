import { defineStore } from "pinia";
import request from "@/utils/request";
import {
  DEFAULT_FOCUS_DAY_BOUNDARY_HOUR,
  normalizeFocusDayBoundaryHour,
} from "@/utils/focusLearningDay";

const FOCUS_DAY_BOUNDARY_STORAGE_KEY = "ll_focus_day_boundary_hour";
const FOCUS_DAY_BOUNDARY_SETTING_KEY = "focus_day_boundary_hour";

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    activeStageId: Number(localStorage.getItem("ll_active_stage_id") || 0),
    focusDayBoundaryHour: normalizeFocusDayBoundaryHour(
      localStorage.getItem(FOCUS_DAY_BOUNDARY_STORAGE_KEY) ??
        DEFAULT_FOCUS_DAY_BOUNDARY_HOUR,
    ),
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

          this.focusDayBoundaryHour = normalizeFocusDayBoundaryHour(
            resolved[FOCUS_DAY_BOUNDARY_SETTING_KEY] ??
              localStorage.getItem(FOCUS_DAY_BOUNDARY_STORAGE_KEY) ??
              DEFAULT_FOCUS_DAY_BOUNDARY_HOUR,
          );
          localStorage.setItem(
            FOCUS_DAY_BOUNDARY_STORAGE_KEY,
            String(this.focusDayBoundaryHour),
          );
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
            [FOCUS_DAY_BOUNDARY_SETTING_KEY]: this.focusDayBoundaryHour,
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
    setFocusDayBoundaryHour(hour: unknown) {
      this.focusDayBoundaryHour = normalizeFocusDayBoundaryHour(hour);
      localStorage.setItem(
        FOCUS_DAY_BOUNDARY_STORAGE_KEY,
        String(this.focusDayBoundaryHour),
      );
      void this.saveSettings();
    },
    setSidebarCollapsed(collapsed: boolean) {
      this.layout.sidebarCollapsed = collapsed;
      localStorage.setItem("ll_sidebar_collapsed", collapsed ? "1" : "0");
    },
  },
});
