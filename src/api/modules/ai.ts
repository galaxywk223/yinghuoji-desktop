import request from "@/utils/request";

export type AiProviderPayload = {
  provider_label?: string;
  base_url?: string;
  model?: string;
  api_key?: string;
};

export type AiProviderModelsPayload = {
  base_url?: string;
  model?: string;
};

export const aiAPI = {
  getProvider() {
    return request({ url: "/api/ai/provider", method: "get" });
  },
  saveProvider(data: AiProviderPayload) {
    return request({ url: "/api/ai/provider", method: "post", data });
  },
  testProvider() {
    return request({ url: "/api/ai/provider/test", method: "post" });
  },
  listProviderModels(data: AiProviderModelsPayload = {}) {
    return request({ url: "/api/ai/provider/models", method: "post", data });
  },
  clearProviderKey() {
    return request({ url: "/api/ai/provider/key", method: "delete" });
  },
  listSessions() {
    return request({ url: "/api/ai/chat/sessions", method: "get" });
  },
  createSession(data: Record<string, any> = {}) {
    return request({ url: "/api/ai/chat/sessions", method: "post", data });
  },
  getSession(sessionId: number) {
    return request({
      url: `/api/ai/chat/sessions/${sessionId}`,
      method: "get",
    });
  },
  deleteSession(sessionId: number) {
    return request({
      url: `/api/ai/chat/sessions/${sessionId}`,
      method: "delete",
    });
  },
  generateSessionTitle(sessionId: number) {
    return request({
      url: `/api/ai/chat/sessions/${sessionId}/title`,
      method: "post",
      timeout: 60000,
    });
  },
  sendMessage(data: { session_id?: number | null; message: string }) {
    return request({
      url: "/api/ai/chat/send",
      method: "post",
      data,
      timeout: 180000,
    });
  },
};
