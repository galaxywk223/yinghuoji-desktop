import { invoke } from "@tauri-apps/api/core";

export const desktopAPI = {
  initialize() {
    return invoke("app_initialize");
  },
  getAiConfig() {
    return invoke("ai_get_config");
  },
  updateAiConfig(payload: {
    api_key?: string;
    model_name?: string;
    base_url?: string;
    enabled?: boolean;
  }) {
    return invoke("ai_update_config", { payload });
  },
};
