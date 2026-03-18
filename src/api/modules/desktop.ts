import { invoke } from "@tauri-apps/api/core";

export const desktopAPI = {
  initialize() {
    return invoke("app_initialize");
  },
  getProfile() {
    return invoke("profile_get");
  },
  updateProfile(payload: { username: string; email?: string }) {
    return invoke("profile_update", { payload });
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
