<template>
  <PageContainer
    :title="{ icon: 'lucide:sparkles', text: 'AI 配置' }"
    subtitle="按需启用在线 AI 能力，核心本地功能不受影响"
    :custom-class="'settings-subpage'"
  >
    <div class="ai-config-grid">
      <section class="config-card">
        <div class="status-row">
          <div>
            <h3>当前状态</h3>
            <p>桌面端会将 API Key 保存到系统安全存储，不写入数据库。</p>
          </div>
          <span class="status-pill" :class="statusClass">{{ statusText }}</span>
        </div>

        <form class="config-form" @submit.prevent="saveConfig">
          <label class="field">
            <span>启用 AI</span>
            <el-switch v-model="form.enabled" />
          </label>

          <label class="field">
            <span>模型名称</span>
            <input
              v-model="form.model_name"
              type="text"
              placeholder="例如 qwen-plus-2025-07-28"
              :disabled="saving"
            />
          </label>

          <label class="field">
            <span>兼容接口地址</span>
            <input
              v-model="form.base_url"
              type="text"
              placeholder="https://dashscope.aliyuncs.com/compatible-mode/v1"
              :disabled="saving"
            />
          </label>

          <label class="field">
            <span>API Key</span>
            <input
              v-model="form.api_key"
              type="password"
              placeholder="留空表示不修改；输入单个空格后保存可清除"
              :disabled="saving"
            />
          </label>

          <div class="actions">
            <button
              type="button"
              class="pill-btn secondary"
              :disabled="saving"
              @click="resetForm"
            >
              重置
            </button>
            <button type="submit" class="pill-btn primary" :disabled="saving">
              {{ saving ? "保存中..." : "保存配置" }}
            </button>
          </div>
        </form>
      </section>

      <section class="config-card tips-card">
        <h3>状态说明</h3>
        <ul>
          <li><strong>未配置</strong>：未保存 API Key，AI 页面可见但不可用。</li>
          <li><strong>网络失败</strong>：配置存在，但请求服务端点失败。</li>
          <li><strong>配额失败</strong>：服务返回限流或额度不足。</li>
        </ul>
      </section>
    </div>
  </PageContainer>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ElMessage } from "element-plus";
import PageContainer from "@/components/layout/PageContainer.vue";
import { desktopAPI } from "@/api/modules/desktop";

defineOptions({ name: "AIConfigView" });

type AiConfigForm = {
  enabled: boolean;
  configured: boolean;
  has_api_key: boolean;
  model_name: string;
  base_url: string;
  api_key: string;
};

const saving = ref(false);
const original = ref<AiConfigForm>({
  enabled: true,
  configured: false,
  has_api_key: false,
  model_name: "",
  base_url: "",
  api_key: "",
});
const form = ref<AiConfigForm>({
  enabled: true,
  configured: false,
  has_api_key: false,
  model_name: "",
  base_url: "",
  api_key: "",
});

const statusText = computed(() => {
  if (!form.value.has_api_key) return "未配置";
  if (!form.value.enabled) return "已停用";
  return "可用";
});

const statusClass = computed(() => {
  if (!form.value.has_api_key) return "is-warning";
  if (!form.value.enabled) return "is-muted";
  return "is-ready";
});

function resetForm() {
  form.value = {
    ...original.value,
    api_key: "",
  };
}

async function loadConfig() {
  const response: any = await desktopAPI.getAiConfig();
  const data = response?.data || {};
  original.value = {
    enabled: !!data.enabled,
    configured: !!data.configured,
    has_api_key: !!data.has_api_key,
    model_name: data.model_name || "",
    base_url: data.base_url || "",
    api_key: "",
  };
  resetForm();
}

async function saveConfig() {
  saving.value = true;
  try {
    const response: any = await desktopAPI.updateAiConfig({
      enabled: form.value.enabled,
      model_name: form.value.model_name.trim(),
      base_url: form.value.base_url.trim(),
      api_key: form.value.api_key,
    });
    ElMessage.success(response?.message || "AI 配置已保存");
    await loadConfig();
  } catch (error) {
    console.error("保存 AI 配置失败:", error);
    ElMessage.error("保存失败，请稍后重试");
  } finally {
    saving.value = false;
  }
}

onMounted(async () => {
  try {
    await loadConfig();
  } catch (error) {
    console.error("加载 AI 配置失败:", error);
    ElMessage.error("加载 AI 配置失败");
  }
});
</script>

<style scoped>
.ai-config-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.5fr) minmax(280px, 1fr);
  gap: 20px;
}

.config-card {
  padding: 24px;
  border-radius: 20px;
  border: 1px solid var(--stroke-soft);
  background: var(--surface-card);
  box-shadow: var(--box-shadow-card);
}

.status-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 20px;
}

.status-row h3,
.tips-card h3 {
  margin: 0 0 6px;
  color: var(--color-text-heading);
}

.status-row p,
.tips-card li {
  color: var(--color-text-secondary);
  line-height: 1.7;
}

.status-pill {
  display: inline-flex;
  align-items: center;
  min-height: 32px;
  padding: 0 12px;
  border-radius: 999px;
  font-size: 13px;
  font-weight: 700;
}

.status-pill.is-ready {
  background: rgba(34, 197, 94, 0.14);
  color: #15803d;
}

.status-pill.is-warning {
  background: rgba(245, 158, 11, 0.14);
  color: #b45309;
}

.status-pill.is-muted {
  background: var(--surface-card-muted);
  color: var(--color-text-secondary);
}

.config-form {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
  color: var(--color-text-heading);
  font-weight: 600;
}

.field input {
  min-height: 44px;
  padding: 0 14px;
  border-radius: 12px;
  border: 1px solid var(--stroke-soft);
  background: var(--surface-card-muted);
  color: var(--color-text-heading);
  outline: none;
}

.field input:focus {
  border-color: var(--color-primary);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 6px;
}

.tips-card ul {
  margin: 0;
  padding-left: 18px;
}

@media (max-width: 900px) {
  .ai-config-grid {
    grid-template-columns: 1fr;
  }

  .status-row {
    flex-direction: column;
  }

  .actions {
    justify-content: stretch;
    flex-direction: column;
  }
}
</style>
