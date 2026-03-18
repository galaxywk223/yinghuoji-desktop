import { invoke } from "@tauri-apps/api/core";
import { ElMessage } from "element-plus";

type RequestConfig = {
  url: string;
  method?: string;
  params?: Record<string, any>;
  data?: any;
  responseType?: string;
  headers?: Record<string, any>;
  timeout?: number;
  maxBodyLength?: number;
};

type RequestMethodConfig = Omit<RequestConfig, "url" | "method" | "data">;
type RequestCallable = {
  (config: RequestConfig): Promise<any>;
  get(url: string, config?: RequestMethodConfig): Promise<any>;
  delete(url: string, config?: RequestMethodConfig): Promise<any>;
  post(url: string, data?: any, config?: RequestMethodConfig): Promise<any>;
  put(url: string, data?: any, config?: RequestMethodConfig): Promise<any>;
};

async function fileToBytes(file: File) {
  const buffer = await file.arrayBuffer();
  return Array.from(new Uint8Array(buffer));
}

async function formDataToFilePayload(formData: FormData) {
  const file = formData.get("file");
  if (!(file instanceof File)) {
    throw new Error("未找到上传文件");
  }
  return {
    fileName: file.name,
    fileBytes: await fileToBytes(file),
  };
}

function blobResponse(payload: any) {
  const bytes = payload?.data || [];
  const fileName = payload?.file_name || "download.bin";
  return {
    data: new Blob([Uint8Array.from(bytes)]),
    headers: {
      "content-disposition": `attachment; filename="${encodeURIComponent(fileName)}"`,
    },
  };
}

function normalizeItems(data: Record<string, any> = {}) {
  return Object.entries(data).map(([key, value]) => ({ key, value }));
}

export function extractRequestErrorMessage(error: any) {
  const candidates = [
    error?.response?.data?.message,
    error?.message,
    error?.error,
    error?.cause?.message,
    typeof error === "string" ? error : null,
  ];

  for (const candidate of candidates) {
    if (typeof candidate === "string" && candidate.trim()) {
      const text = candidate.trim();
      if (text.startsWith("{") && text.endsWith("}")) {
        try {
          const parsed = JSON.parse(text);
          if (typeof parsed?.message === "string" && parsed.message.trim()) {
            return parsed.message.trim();
          }
        } catch {
          return text;
        }
      }
      return text;
    }
  }

  if (error && typeof error === "object") {
    if (typeof error.message?.message === "string" && error.message.message.trim()) {
      return error.message.message.trim();
    }
    if (typeof error.error?.message === "string" && error.error.message.trim()) {
      return error.error.message.trim();
    }
  }

  return "请求失败";
}

async function baseRequest(config: RequestConfig) {
  const method = (config.method || "get").toLowerCase();
  const { url, params = {}, data } = config;

  try {
    if (url === "/api/auth/me" && method === "get") {
      return await invoke("profile_get");
    }
    if (url === "/api/auth/login" && method === "post") {
      return {
        success: false,
        message: "桌面端为单用户本地模式，不支持登录。",
      };
    }
    if (url === "/api/auth/register" && method === "post") {
      return {
        success: false,
        message: "桌面端为单用户本地模式，不支持注册。",
      };
    }
    if (url === "/api/auth/refresh" && method === "post") {
      return { success: true, access_token: "local-mode-token" };
    }
    if (url === "/api/auth/change-password" && method === "post") {
      return {
        success: false,
        message: "桌面端为本地档案模式，不提供密码修改。",
      };
    }

    if (url === "/api/stages" && method === "get") {
      return await invoke("stages_list");
    }
    if (url === "/api/stages" && method === "post") {
      return await invoke("stage_create", { payload: data });
    }
    if (/^\/api\/stages\/\d+$/.test(url)) {
      const stageId = Number(url.split("/").pop());
      if (method === "get") return await invoke("stage_get", { stageId });
      if (method === "put") return await invoke("stage_update", { stageId, payload: data });
      if (method === "delete") return await invoke("stage_delete", { stageId });
    }

    if (url === "/api/categories" && method === "get") {
      const includeSubcategories =
        params.include_subcategories === true ||
        params.include_subcategories === "true";
      return await invoke("categories_list", { includeSubcategories });
    }
    if (url === "/api/categories" && method === "post") {
      return await invoke("category_create", { payload: data });
    }
    if (/^\/api\/categories\/\d+$/.test(url)) {
      const categoryId = Number(url.split("/").pop());
      if (method === "get") return await invoke("category_get", { categoryId });
      if (method === "put") return await invoke("category_update", { categoryId, payload: data });
      if (method === "delete") return await invoke("category_delete", { categoryId });
    }
    if (/^\/api\/categories\/\d+\/subcategories$/.test(url) && method === "post") {
      const categoryId = Number(url.split("/")[3]);
      return await invoke("subcategory_create", { categoryId, payload: data });
    }
    if (/^\/api\/categories\/subcategories\/\d+$/.test(url)) {
      const subcategoryId = Number(url.split("/").pop());
      if (method === "put") {
        return await invoke("subcategory_update", { subcategoryId, payload: data });
      }
      if (method === "delete") {
        return await invoke("subcategory_delete", { subcategoryId });
      }
    }
    if (/^\/api\/categories\/subcategories\/\d+\/merge$/.test(url) && method === "post") {
      const subcategoryId = Number(url.split("/")[4]);
      return await invoke("subcategory_merge", { subcategoryId, payload: data });
    }

    if (url === "/api/records/structured" && method === "get") {
      return await invoke("records_structured", { query: params });
    }
    if (url === "/api/records/list" && method === "get") {
      return await invoke("records_list", { query: params });
    }
    if (url === "/api/records/recent" && method === "get") {
      return await invoke("records_recent", { query: params });
    }
    if ((url === "/api/records/stats" || url === "/api/records/statistics") && method === "get") {
      return await invoke("record_statistics", { query: params });
    }
    if ((url === "/api/records" || url === "/api/records/") && method === "post") {
      return await invoke("record_create", { payload: data });
    }
    if (/^\/api\/records\/\d+$/.test(url)) {
      const recordId = Number(url.split("/").pop());
      if (method === "get") return await invoke("record_get", { recordId });
      if (method === "put") return await invoke("record_update", { recordId, payload: data });
      if (method === "delete") return await invoke("record_delete", { recordId });
    }
    if (url === "/api/records/export" && method === "get") {
      const result = await invoke<any>("backup_export_zip");
      return config.responseType === "blob" ? blobResponse(result) : result;
    }
    if (
      (url === "/api/records/import" || url === "/api/records/import_zip") &&
      method === "post"
    ) {
      const payload = await formDataToFilePayload(data);
      return await invoke("backup_import_zip", payload);
    }
    if (
      (url === "/api/records/clear-all" ||
        url === "/api/records/clear_data" ||
        url === "/api/records/clear") &&
      (method === "post" || method === "delete")
    ) {
      return await invoke("backup_clear_all");
    }

    if (url === "/api/users/dashboard/summary" && method === "get") {
      return await invoke("dashboard_summary");
    }
    if (url === "/api/users/settings" && method === "get") {
      return await invoke("settings_get");
    }
    if (url === "/api/users/settings" && method === "post") {
      return await invoke("settings_set", { items: normalizeItems(data) });
    }
    if (url === "/api/users/profile" && method === "get") {
      return await invoke("profile_get");
    }
    if (url === "/api/users/profile" && method === "put") {
      return await invoke("profile_update", { payload: data });
    }

    if (url === "/api/charts/overview" && method === "get") {
      return await invoke("charts_overview", { query: params });
    }
    if (url === "/api/charts/categories" && method === "get") {
      return await invoke("charts_categories", { query: params });
    }
    if (url === "/api/charts/category_trend" && method === "get") {
      return await invoke("charts_category_trend", { query: params });
    }
    if (url === "/api/charts/stages" && method === "get") {
      return await invoke("charts_stages");
    }
    if (url === "/api/charts/overview_forecast" && method === "get") {
      return await invoke("charts_overview_forecast_status");
    }
    if (url === "/api/charts/overview_forecast/retrain" && method === "post") {
      return await invoke("charts_overview_forecast_retrain");
    }

    if (url === "/api/countdowns" && method === "get") {
      return await invoke("countdowns_list");
    }
    if (url === "/api/countdowns" && method === "post") {
      return await invoke("countdown_create", { payload: data });
    }
    if (/^\/api\/countdowns\/\d+$/.test(url)) {
      const countdownId = Number(url.split("/").pop());
      if (method === "get") return await invoke("countdown_get", { countdownId });
      if (method === "put") return await invoke("countdown_update", { countdownId, payload: data });
      if (method === "delete") return await invoke("countdown_delete", { countdownId });
    }

    if (url === "/api/mottos" && method === "get") {
      return await invoke("mottos_list");
    }
    if (url === "/api/mottos/random" && method === "get") {
      return await invoke("motto_random");
    }
    if (url === "/api/mottos" && method === "post") {
      return await invoke("motto_create", { payload: data });
    }
    if (/^\/api\/mottos\/\d+$/.test(url)) {
      const mottoId = Number(url.split("/").pop());
      if (method === "get") return await invoke("motto_get", { mottoId });
      if (method === "put") return await invoke("motto_update", { mottoId, payload: data });
      if (method === "delete") return await invoke("motto_delete", { mottoId });
    }

    if (url === "/api/milestones" && method === "get") {
      return await invoke("milestones_list", { query: params });
    }
    if (url === "/api/milestones" && method === "post") {
      return await invoke("milestone_create", { payload: data });
    }
    if (url === "/api/milestones/categories" && method === "get") {
      return await invoke("milestone_categories_list");
    }
    if (url === "/api/milestones/categories" && method === "post") {
      return await invoke("milestone_category_create", { payload: data });
    }
    if (/^\/api\/milestones\/categories\/\d+$/.test(url)) {
      const categoryId = Number(url.split("/").pop());
      if (method === "put") {
        return await invoke("milestone_category_update", { categoryId, payload: data });
      }
      if (method === "delete") {
        return await invoke("milestone_category_delete", { categoryId });
      }
    }
    if (/^\/api\/milestones\/\d+$/.test(url)) {
      const milestoneId = Number(url.split("/").pop());
      if (method === "get") return await invoke("milestone_get", { milestoneId });
      if (method === "put") return await invoke("milestone_update", { milestoneId, payload: data });
      if (method === "delete") return await invoke("milestone_delete", { milestoneId });
    }
    if (/^\/api\/milestones\/\d+\/attachments$/.test(url) && method === "post") {
      const milestoneId = Number(url.split("/")[3]);
      const payload = await formDataToFilePayload(data);
      return await invoke("milestone_attachment_upload", { milestoneId, ...payload });
    }
    if (/^\/api\/milestones\/\d+\/attachments\/\d+$/.test(url) && method === "delete") {
      const parts = url.split("/");
      const milestoneId = Number(parts[3]);
      const attachmentId = Number(parts[5]);
      return await invoke("milestone_attachment_delete", { milestoneId, attachmentId });
    }
    if (url.startsWith("/api/milestones/attachments/") && method === "get") {
      const filePath = decodeURIComponent(url.replace("/api/milestones/attachments/", ""));
      const result = await invoke<any>("milestone_attachment_get", { filePath });
      return config.responseType === "blob" ? blobResponse(result) : result;
    }

    if (url === "/api/ai/chat/messages" && method === "post") {
      return await invoke("ai_chat_send", { payload: data });
    }
    if (url === "/api/ai/chat/sessions" && method === "get") {
      return await invoke("ai_chat_sessions");
    }
    if (/^\/api\/ai\/chat\/sessions\/\d+\/messages$/.test(url) && method === "get") {
      const sessionId = Number(url.split("/")[5]);
      return await invoke("ai_chat_messages", { sessionId });
    }
    if (url === "/api/ai/history" && method === "get") {
      return await invoke("ai_history_list", { query: params });
    }

    if (url.startsWith("/api/leaderboard")) {
      return {
        success: false,
        message: "桌面端当前版本未提供社区排行。",
        data: [],
      };
    }

    throw new Error(`Unsupported request: ${method.toUpperCase()} ${url}`);
  } catch (error: any) {
    const message = extractRequestErrorMessage(error);
    ElMessage.error(message);
    throw error;
  }
}

const request = Object.assign(baseRequest, {
  get(url: string, config: RequestMethodConfig = {}) {
    return baseRequest({ ...config, url, method: "get" });
  },
  delete(url: string, config: RequestMethodConfig = {}) {
    return baseRequest({ ...config, url, method: "delete" });
  },
  post(url: string, data?: any, config: RequestMethodConfig = {}) {
    return baseRequest({ ...config, url, data, method: "post" });
  },
  put(url: string, data?: any, config: RequestMethodConfig = {}) {
    return baseRequest({ ...config, url, data, method: "put" });
  },
}) as RequestCallable;

export default request;
