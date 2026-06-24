import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { ElMessage } from "element-plus";
import {
  aiAPI,
  type AiProviderModelsPayload,
  type AiProviderPayload,
} from "@/api/modules/ai";

export type AiSession = {
  id: number;
  title: string;
  scope: string;
  created_at: string;
  updated_at: string;
  last_message_at: string;
};

export type AiMessage = {
  id: number;
  session_id: number;
  role: "user" | "assistant" | "system";
  content: string;
  generation_mode?: string | null;
  model_name?: string | null;
  created_at: string;
  meta?: Record<string, any>;
};

export type AiToolAudit = {
  name: string;
  arguments: Record<string, any>;
  row_count: number;
  truncated: boolean;
  error?: string | null;
};

export type AiProvider = {
  provider_label: string;
  base_url: string;
  model: string;
  has_api_key: boolean;
};

export type AiProviderModel = {
  id: string;
  label: string;
  object?: string | null;
  created?: number | null;
  owned_by?: string | null;
};

export const useAiStore = defineStore("ai", () => {
  const sessions = ref<AiSession[]>([]);
  const messages = ref<AiMessage[]>([]);
  const activeSession = ref<AiSession | null>(null);
  const provider = ref<AiProvider | null>(null);
  const providerModels = ref<AiProviderModel[]>([]);
  const modelListStatus = ref<"idle" | "ok" | "error">("idle");
  const modelListError = ref("");
  const toolAudits = ref<AiToolAudit[]>([]);
  const loadingSessions = ref(false);
  const loadingMessages = ref(false);
  const sending = ref(false);
  const providerLoading = ref(false);
  const testingProvider = ref(false);
  const loadingProviderModels = ref(false);
  const generatingTitleSessionId = ref<number | null>(null);

  const hasProviderKey = computed(() => Boolean(provider.value?.has_api_key));
  const activeSessionId = computed(() => activeSession.value?.id ?? null);

  async function fetchProvider() {
    providerLoading.value = true;
    try {
      const res: any = await aiAPI.getProvider();
      provider.value = res?.provider || null;
    } finally {
      providerLoading.value = false;
    }
  }

  async function saveProvider(payload: AiProviderPayload) {
    providerLoading.value = true;
    try {
      const res: any = await aiAPI.saveProvider(payload);
      provider.value = res?.provider || provider.value;
      providerModels.value = res?.models || providerModels.value;
      modelListStatus.value = res?.model_list_status || modelListStatus.value;
      modelListError.value = res?.model_list_error || "";
      ElMessage.success(res?.message || "AI 设置已保存");
    } finally {
      providerLoading.value = false;
    }
  }

  async function clearProviderKey() {
    providerLoading.value = true;
    try {
      const res: any = await aiAPI.clearProviderKey();
      provider.value = res?.provider || provider.value;
      providerModels.value = [];
      modelListStatus.value = "idle";
      modelListError.value = "";
      ElMessage.success(res?.message || "API Key 已清除");
    } finally {
      providerLoading.value = false;
    }
  }

  async function testProvider() {
    testingProvider.value = true;
    try {
      const res: any = await aiAPI.testProvider();
      ElMessage.success(res?.message || "连接测试成功");
      return res;
    } finally {
      testingProvider.value = false;
    }
  }

  async function refreshProviderModels(payload: AiProviderModelsPayload = {}) {
    loadingProviderModels.value = true;
    modelListError.value = "";
    try {
      const res: any = await aiAPI.listProviderModels(payload);
      provider.value = res?.provider || provider.value;
      providerModels.value = res?.models || [];
      modelListStatus.value = res?.model_list_status || "ok";
      modelListError.value = res?.model_list_error || "";
      ElMessage.success(res?.message || "模型列表已刷新");
      return res;
    } catch (error: any) {
      modelListStatus.value = "error";
      modelListError.value = error?.message || "模型列表获取失败";
      throw error;
    } finally {
      loadingProviderModels.value = false;
    }
  }

  async function fetchSessions() {
    loadingSessions.value = true;
    try {
      const res: any = await aiAPI.listSessions();
      sessions.value = res?.sessions || [];
      if (activeSession.value) {
        activeSession.value =
          sessions.value.find((item) => item.id === activeSession.value?.id) ||
          activeSession.value;
      }
    } finally {
      loadingSessions.value = false;
    }
  }

  async function createSession(
    payload: Record<string, any> = { title: "新的对话" },
  ) {
    const res: any = await aiAPI.createSession(payload);
    const session = res?.session;
    if (session) {
      activeSession.value = session;
      messages.value = [];
      await fetchSessions();
    }
    return session;
  }

  async function selectSession(sessionId: number) {
    loadingMessages.value = true;
    try {
      const res: any = await aiAPI.getSession(sessionId);
      activeSession.value = res?.session || null;
      messages.value = res?.messages || [];
      const assistantMessages = messages.value.filter(
        (item) => item.role === "assistant",
      );
      toolAudits.value =
        assistantMessages[assistantMessages.length - 1]?.meta?.tool_audits ||
        [];
    } finally {
      loadingMessages.value = false;
    }
  }

  async function deleteSession(sessionId: number) {
    await aiAPI.deleteSession(sessionId);
    if (activeSession.value?.id === sessionId) {
      activeSession.value = null;
      messages.value = [];
      toolAudits.value = [];
    }
    await fetchSessions();
    ElMessage.success("AI 会话已删除");
  }

  async function generateSessionTitle(sessionId: number) {
    if (generatingTitleSessionId.value === sessionId) return null;
    generatingTitleSessionId.value = sessionId;
    try {
      const res: any = await aiAPI.generateSessionTitle(sessionId);
      const session = res?.session;
      if (session) {
        sessions.value = sessions.value.map((item) =>
          item.id === session.id ? { ...item, ...session } : item,
        );
        if (activeSession.value?.id === session.id) {
          activeSession.value = { ...activeSession.value, ...session };
        }
      }
      return res;
    } finally {
      generatingTitleSessionId.value = null;
    }
  }

  async function sendMessage(message: string) {
    const text = message.trim();
    if (!text || sending.value) return null;
    const shouldAutoTitle =
      !activeSession.value ||
      activeSession.value.title === "新的对话" ||
      activeSession.value.title.endsWith("...");
    sending.value = true;
    const optimistic: AiMessage = {
      id: Date.now(),
      session_id: activeSession.value?.id || 0,
      role: "user",
      content: text,
      created_at: new Date().toISOString(),
    };
    messages.value.push(optimistic);
    try {
      const res: any = await aiAPI.sendMessage({
        session_id: activeSession.value?.id || null,
        message: text,
      });
      activeSession.value = res?.session || activeSession.value;
      const returnedMessages = res?.messages || [];
      if (returnedMessages.length) {
        messages.value = messages.value.filter((item) => item.id !== optimistic.id);
        messages.value.push(...returnedMessages);
      }
      toolAudits.value = res?.tool_audits || [];
      await fetchSessions();
      const sessionId = activeSession.value?.id;
      if (shouldAutoTitle && sessionId) {
        void generateSessionTitle(sessionId)
          .then(() => fetchSessions())
          .catch(() => undefined);
      }
      return res;
    } catch (error) {
      messages.value = messages.value.filter((item) => item.id !== optimistic.id);
      throw error;
    } finally {
      sending.value = false;
    }
  }

  return {
    sessions,
    messages,
    activeSession,
    provider,
    providerModels,
    modelListStatus,
    modelListError,
    toolAudits,
    loadingSessions,
    loadingMessages,
    sending,
    providerLoading,
    testingProvider,
    loadingProviderModels,
    generatingTitleSessionId,
    hasProviderKey,
    activeSessionId,
    fetchProvider,
    saveProvider,
    clearProviderKey,
    testProvider,
    refreshProviderModels,
    fetchSessions,
    createSession,
    selectSession,
    deleteSession,
    generateSessionTitle,
    sendMessage,
  };
});
