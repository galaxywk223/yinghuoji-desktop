<template>
  <PageContainer
    :title="{ icon: 'lucide:info', text: '关于与更新' }"
    subtitle="查看当前安装版本，并在正式安装版中手动检查桌面更新。"
    :custom-class="'settings-subpage'"
    max-width="full"
    fill-height
  >
    <div class="about-update-grid">
      <section class="about-card app-meta-card">
        <div class="card-head">
          <div class="card-icon">
            <Icon icon="lucide:sparkles" />
          </div>
          <div>
            <p class="eyebrow">应用信息</p>
            <h3>{{ appInfo.name }}</h3>
          </div>
        </div>

        <dl class="meta-list">
          <div class="meta-item">
            <dt>当前版本</dt>
            <dd>v{{ appInfo.version }}</dd>
          </div>
          <div class="meta-item">
            <dt>运行环境</dt>
            <dd>{{ runtimeLabel }}</dd>
          </div>
          <div class="meta-item">
            <dt>更新支持</dt>
            <dd>{{ appInfo.supportsUpdateCheck ? "已启用" : "不可用" }}</dd>
          </div>
        </dl>

        <p class="meta-note">
          版本号来自桌面端应用元数据。正式安装版支持在线检查更新，开发环境与非
          Tauri 运行环境仅展示当前版本信息。
        </p>
      </section>

      <section class="about-card update-card">
        <div class="card-head">
          <div class="card-icon warm">
            <Icon icon="lucide:download" />
          </div>
          <div>
            <p class="eyebrow">更新机制</p>
            <h3>检查桌面端更新</h3>
          </div>
        </div>

        <ul class="update-points">
          <li>启动后自动检查继续保留，未发现更新时保持静默。</li>
          <li>手动检查会复用同一 updater 流程，发现新版本时继续沿用现有确认与安装逻辑。</li>
          <li>当前已是最新版本时显示轻量提示，不打断当前使用流程。</li>
        </ul>

        <div class="update-actions">
          <el-button
            type="primary"
            size="large"
            :loading="checking"
            :disabled="!appInfo.supportsUpdateCheck"
            @click="handleManualCheck"
          >
            检查更新
          </el-button>
          <p v-if="appInfo.supportsUpdateCheck" class="support-note">
            仅在正式安装版中执行真实更新检查。
          </p>
          <p v-else class="support-note support-note--muted">
            仅正式安装版支持检查更新。
          </p>
        </div>
      </section>

      <section class="about-card runtime-card">
        <div class="card-head">
          <div class="card-icon neutral">
            <Icon icon="lucide:activity" />
          </div>
          <div>
            <p class="eyebrow">运行状态</p>
            <h3>当前环境说明</h3>
          </div>
        </div>

        <div class="runtime-grid">
          <div class="runtime-item">
            <span>版本来源</span>
            <strong>桌面端元数据</strong>
          </div>
          <div class="runtime-item">
            <span>更新检查</span>
            <strong>{{ appInfo.supportsUpdateCheck ? "可执行" : "仅展示" }}</strong>
          </div>
          <div class="runtime-item">
            <span>当前环境</span>
            <strong>{{ runtimeLabel }}</strong>
          </div>
        </div>

        <p class="meta-note">
          开发版用于验证界面与功能流程。正式安装版会继续使用当前 updater
          配置完成在线检查、确认和安装。
        </p>
      </section>
    </div>
  </PageContainer>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Icon } from "@iconify/vue";
import PageContainer from "@/components/layout/PageContainer.vue";
import {
  checkForUpdates,
  getDesktopAppInfo,
  getDesktopAppInfoFallback,
  type DesktopAppInfo,
} from "@/utils/updater";

defineOptions({ name: "AboutUpdateSettingsView" });

const appInfo = ref<DesktopAppInfo>(getDesktopAppInfoFallback());
const checking = ref(false);

const runtimeLabel = computed(() => {
  if (appInfo.value.supportsUpdateCheck) {
    return "正式安装版";
  }

  if (appInfo.value.isDesktopRuntime) {
    return "桌面开发环境";
  }

  return "浏览器预览环境";
});

async function loadAppInfo() {
  appInfo.value = await getDesktopAppInfo();
}

async function handleManualCheck() {
  if (checking.value || !appInfo.value.supportsUpdateCheck) {
    return;
  }

  checking.value = true;
  try {
    await checkForUpdates({ source: "manual" });
  } finally {
    checking.value = false;
  }
}

onMounted(() => {
  void loadAppInfo();
});
</script>

<style scoped>
.about-update-grid {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 24px;
  align-items: stretch;
}

.about-card {
  min-width: 0;
  height: 100%;
  padding: 24px;
  border-radius: 22px;
  border: 1px solid var(--stroke-soft);
  background:
    radial-gradient(circle at top right, rgba(64, 158, 255, 0.12), transparent 34%),
    linear-gradient(180deg, var(--surface-card) 0%, var(--surface-card-muted) 100%);
  box-shadow: var(--box-shadow-card);
  display: flex;
  flex-direction: column;
  gap: 20px;
  min-height: 300px;
}

.card-head {
  display: flex;
  align-items: center;
  gap: 14px;
}

.card-icon {
  width: 52px;
  height: 52px;
  border-radius: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: rgba(64, 158, 255, 0.14);
  color: var(--color-primary);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.75);
}

.card-icon :deep(svg) {
  width: 24px;
  height: 24px;
}

.card-icon.warm {
  background: rgba(245, 158, 11, 0.14);
  color: #d97706;
}

.card-icon.neutral {
  background: var(--surface-card-muted);
  color: var(--color-text-secondary);
}

.eyebrow {
  margin: 0 0 6px;
  font-size: 12px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--color-text-muted);
}

.card-head h3 {
  margin: 0;
  font-size: 22px;
  color: var(--color-text-heading);
}

.meta-list {
  display: grid;
  gap: 12px;
  margin: 0;
}

.meta-item {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 16px;
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.58);
  border: 1px solid var(--stroke-soft);
}

.meta-item dt {
  color: var(--color-text-secondary);
  font-size: 13px;
}

.meta-item dd {
  margin: 0;
  font-size: 14px;
  font-weight: 700;
  color: var(--color-text-heading);
}

.meta-note,
.support-note {
  margin: 0;
  font-size: 13px;
  line-height: 1.7;
  color: var(--color-text-secondary);
}

.support-note--muted {
  color: var(--color-warning);
}

.update-points {
  margin: 0;
  padding-left: 18px;
  display: grid;
  gap: 10px;
  color: var(--color-text-secondary);
  line-height: 1.7;
}

.update-actions {
  margin-top: auto;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 12px;
}

.runtime-card {
  grid-column: 1 / -1;
  min-height: 180px;
}

.runtime-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.runtime-item {
  padding: 14px 16px;
  border: 1px solid var(--stroke-soft);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.48);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.runtime-item span {
  color: var(--color-text-muted);
  font-size: 12px;
}

.runtime-item strong {
  color: var(--color-text-heading);
  font-size: 15px;
}

@media (max-width: 900px) {
  .about-update-grid {
    grid-template-columns: 1fr;
  }

  .runtime-grid {
    grid-template-columns: 1fr;
  }
}
</style>
