<template>
  <PageContainer
    :title="{ icon: 'lucide:key-round', text: 'AI 设置' }"
    subtitle="连接 OpenAI 兼容模型服务，供本地数据助手使用"
    :custom-class="'settings-subpage'"
    max-width="full"
    fill-height
  >
    <div class="ai-settings-grid">
      <section class="settings-card">
        <div class="section-head">
          <div>
            <span class="section-label">Provider</span>
            <h2>模型接口</h2>
          </div>
          <el-tag :type="store.hasProviderKey ? 'success' : 'warning'" effect="plain">
            {{ store.hasProviderKey ? "已保存 Key" : "未保存 Key" }}
          </el-tag>
        </div>

        <el-form label-position="top" class="provider-form">
          <el-form-item label="Base URL">
            <el-input v-model="form.base_url" placeholder="https://api.openai.com/v1" />
          </el-form-item>
          <el-form-item label="API Key">
            <el-input
              v-model="form.api_key"
              type="password"
              show-password
              placeholder="留空表示不修改已保存的 Key"
            />
          </el-form-item>
          <el-form-item label="当前模型">
            <div class="model-row">
              <el-select
                v-model="form.model"
                class="model-select"
                filterable
                allow-create
                default-first-option
                placeholder="保存 Key 后自动获取模型"
              >
                <el-option
                  v-for="item in modelOptions"
                  :key="item.id"
                  :label="item.label || item.id"
                  :value="item.id"
                />
              </el-select>
              <button
                class="icon-btn"
                type="button"
                :disabled="store.loadingProviderModels || !store.hasProviderKey"
                title="刷新模型列表"
                @click="refreshModels"
              >
                <Icon icon="lucide:refresh-cw" />
              </button>
            </div>
            <p class="field-note">
              {{ modelStatusText }}
            </p>
          </el-form-item>
        </el-form>

        <div class="action-row">
          <button
            class="pill-btn primary"
            type="button"
            :disabled="store.providerLoading"
            @click="save"
          >
            <Icon icon="lucide:save" />
            保存并识别模型
          </button>
          <button
            class="pill-btn secondary"
            type="button"
            :disabled="store.testingProvider || !store.hasProviderKey"
            @click="test"
          >
            <Icon icon="lucide:plug-zap" />
            测试连接
          </button>
          <button
            class="pill-btn danger"
            type="button"
            :disabled="store.providerLoading || !store.hasProviderKey"
            @click="clearKey"
          >
            <Icon icon="lucide:key-round-x" />
            清除 Key
          </button>
        </div>
      </section>

      <aside class="settings-card support-card">
        <section>
          <span class="section-label">模型选择</span>
          <h3>自动读取可用模型</h3>
          <p>保存接口和 Key 后，应用会从兼容接口读取模型列表，并优先保留当前可用模型。</p>
        </section>
        <section>
          <span class="section-label">参数策略</span>
          <h3>使用服务默认生成参数</h3>
          <p>应用只提交对话、模型和工具定义，其他生成细节由模型服务处理。</p>
        </section>
        <section>
          <span class="section-label">本地数据</span>
          <h3>按需读取应用数据</h3>
          <p>AI 助手仅在模型调用只读工具时读取本地数据库，并把相关查询结果发送给模型服务。</p>
        </section>
      </aside>
    </div>
  </PageContainer>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, watch } from "vue";
import { Icon } from "@iconify/vue";
import { ElMessageBox } from "element-plus";
import PageContainer from "@/components/layout/PageContainer.vue";
import { useAiStore } from "@/stores/modules/ai";

const store = useAiStore();

const form = reactive({
  provider_label: "OpenAI Compatible",
  base_url: "https://api.openai.com/v1",
  model: "",
  api_key: "",
});

const modelOptions = computed(() => {
  const items = [...store.providerModels];
  if (form.model && !items.some((item) => item.id === form.model)) {
    items.unshift({ id: form.model, label: form.model });
  }
  return items;
});

const modelStatusText = computed(() => {
  if (store.loadingProviderModels) return "正在读取模型列表...";
  if (store.modelListStatus === "error") {
    return store.modelListError || "模型列表读取失败，可保留当前模型后测试连接。";
  }
  if (store.providerModels.length) {
    return `已读取 ${store.providerModels.length} 个模型。`;
  }
  if (store.hasProviderKey) return "已保存 Key，可刷新模型列表或直接测试连接。";
  return "保存 API Key 后自动获取模型列表。";
});

function syncForm() {
  const provider = store.provider;
  if (!provider) return;
  form.provider_label = provider.provider_label || "OpenAI Compatible";
  form.base_url = provider.base_url || "https://api.openai.com/v1";
  form.model = provider.model || "";
  form.api_key = "";
}

async function save() {
  await store.saveProvider({
    provider_label: form.provider_label,
    base_url: form.base_url,
    model: form.model || undefined,
    api_key: form.api_key,
  });
  form.api_key = "";
  syncForm();
}

async function refreshModels() {
  const res = await store.refreshProviderModels({
    base_url: form.base_url,
    model: form.model,
  });
  if (res?.provider) syncForm();
}

async function test() {
  await store.testProvider();
}

async function clearKey() {
  try {
    await ElMessageBox.confirm("确认从系统凭据库中清除当前 API Key 吗？", "清除 API Key", {
      type: "warning",
      confirmButtonText: "清除",
      cancelButtonText: "取消",
    });
  } catch {
    return;
  }
  await store.clearProviderKey();
}

watch(
  () => store.provider,
  () => syncForm(),
);

onMounted(async () => {
  await store.fetchProvider();
  syncForm();
});
</script>

<style scoped lang="scss">
.ai-settings-grid {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 320px;
  gap: 16px;
  align-items: start;
}

.settings-card {
  border: 1px solid var(--stroke-soft);
  border-radius: 8px;
  background: var(--surface-card);
  box-shadow: var(--box-shadow-card);
  padding: 18px;
  min-width: 0;
}

.section-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
}

.section-label {
  display: block;
  margin-bottom: 5px;
  color: var(--color-text-muted);
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.section-head h2,
.support-card h3 {
  margin: 0;
  color: var(--color-text-heading);
  font-size: 17px;
  font-weight: 800;
}

.provider-form {
  max-width: 760px;
}

.model-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.model-select {
  flex: 1;
  min-width: 0;
}

.field-note {
  margin: 7px 0 0;
  color: var(--color-text-muted);
  font-size: 12px;
  line-height: 1.5;
}

.action-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  margin-top: 8px;
}

.pill-btn,
.icon-btn {
  min-height: 38px;
  border-radius: 8px;
  border: 1px solid var(--stroke-soft);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: var(--surface-card-muted);
  color: var(--color-text-heading);
  font-weight: 700;
  font-size: 14px;
  cursor: pointer;
}

.pill-btn {
  padding: 0 13px;
  gap: 7px;
}

.icon-btn {
  width: 38px;
  padding: 0;
  flex: 0 0 38px;
}

.pill-btn.primary {
  border-color: var(--color-primary);
  background: var(--color-primary);
  color: var(--color-text-inverse);
}

.pill-btn.danger {
  border-color: rgba(239, 68, 68, 0.28);
  background: rgba(239, 68, 68, 0.12);
  color: var(--color-error);
}

.pill-btn:disabled,
.icon-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.pill-btn svg,
.icon-btn svg {
  width: 16px;
  height: 16px;
}

.support-card {
  display: grid;
  gap: 12px;
}

.support-card section {
  padding: 14px;
  border: 1px solid var(--stroke-soft);
  border-radius: 8px;
  background: var(--surface-card-muted);
}

.support-card p {
  margin: 8px 0 0;
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1.65;
}

@media (max-width: 960px) {
  .ai-settings-grid {
    grid-template-columns: 1fr;
  }
}
</style>
