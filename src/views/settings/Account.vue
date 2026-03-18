<template>
  <PageContainer
    :title="{ icon: 'lucide:user-round', text: '本地档案' }"
    subtitle="管理当前设备上的单用户资料与应用模式"
    :custom-class="'settings-subpage'"
  >
    <div class="account-container">
      <div class="account-header">
        <div class="avatar-wrapper">
          <div class="avatar-preview">
            {{ profileInitial }}
          </div>
        </div>
        <div class="header-info">
          <h3>{{ profile.username || "学习者" }}</h3>
          <p>{{ profile.email || "可选邮箱，仅用于 AI 请求上下文" }}</p>
        </div>
      </div>

      <div class="account-body">
        <div class="form-section">
          <h4>本地资料</h4>
          <form @submit.prevent="handleProfileSubmit">
            <div class="ios-input-group">
              <div class="input-row">
                <label for="username">昵称</label>
                <input
                  id="username"
                  v-model="profileForm.username"
                  type="text"
                  placeholder="给自己起一个名字"
                  :disabled="profileLoading"
                />
              </div>
              <div class="input-row">
                <label for="email">邮箱</label>
                <input
                  id="email"
                  v-model="profileForm.email"
                  type="email"
                  placeholder="可选，用于 AI 上下文"
                  :disabled="profileLoading"
                />
              </div>
            </div>
            <div class="form-actions">
              <div class="pill-btn-group-horizontal">
                <button
                  type="button"
                  class="pill-btn secondary"
                  :disabled="profileLoading || !isProfileChanged"
                  @click="resetProfileForm"
                >
                  取消
                </button>
                <button
                  type="submit"
                  class="pill-btn primary"
                  :disabled="profileLoading || !isProfileChanged"
                >
                  {{ profileLoading ? "保存中..." : "保存更改" }}
                </button>
              </div>
            </div>
          </form>
        </div>

        <div class="form-section">
          <h4>应用模式</h4>
          <div class="meta-list">
            <div class="meta-item">
              <span class="meta-label">数据模式</span>
              <strong>本地优先 / 单用户</strong>
            </div>
            <div class="meta-item">
              <span class="meta-label">身份系统</span>
              <strong>已移除登录与 JWT</strong>
            </div>
            <div class="meta-item">
              <span class="meta-label">核心数据</span>
              <strong>SQLite + 本地附件目录</strong>
            </div>
            <div class="meta-item">
              <span class="meta-label">联网能力</span>
              <strong>仅 AI 功能需要网络</strong>
            </div>
          </div>
        </div>
      </div>
    </div>
  </PageContainer>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ElMessage } from "element-plus";
import PageContainer from "@/components/layout/PageContainer.vue";
import { desktopAPI } from "@/api/modules/desktop";
import { useAuthStore } from "@/stores/modules/auth";

defineOptions({ name: "AccountView" });

const authStore = useAuthStore();
const profileLoading = ref(false);
const profile = ref({
  username: "",
  email: "",
});
const profileForm = ref({
  username: "",
  email: "",
});

const profileInitial = computed(
  () => profileForm.value.username?.trim()?.charAt(0)?.toUpperCase() || "Y",
);

const isProfileChanged = computed(
  () =>
    profileForm.value.username !== profile.value.username ||
    profileForm.value.email !== profile.value.email,
);

function resetProfileForm() {
  profileForm.value = {
    username: profile.value.username,
    email: profile.value.email,
  };
}

async function loadProfile() {
  const response: any = await desktopAPI.getProfile();
  const user = response?.user || response?.data || {};
  profile.value = {
    username: user.username || "",
    email: user.email || "",
  };
  authStore.user = user;
  resetProfileForm();
}

async function handleProfileSubmit() {
  if (!profileForm.value.username.trim()) {
    ElMessage.warning("昵称不能为空");
    return;
  }

  profileLoading.value = true;
  try {
    const response: any = await desktopAPI.updateProfile({
      username: profileForm.value.username.trim(),
      email: profileForm.value.email.trim(),
    });
    const user = response?.user || response?.data || {};
    profile.value = {
      username: user.username || profileForm.value.username.trim(),
      email: user.email || profileForm.value.email.trim(),
    };
    authStore.user = user;
    resetProfileForm();
    ElMessage.success(response?.message || "本地档案已更新");
  } catch (error) {
    console.error("更新本地档案失败:", error);
    ElMessage.error("更新失败，请稍后重试");
  } finally {
    profileLoading.value = false;
  }
}

onMounted(async () => {
  try {
    await loadProfile();
  } catch (error) {
    console.error("加载本地档案失败:", error);
    ElMessage.error("加载本地档案失败");
  }
});
</script>

<style scoped>
@import "@/styles/views/settings/account.scss";

.meta-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.meta-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 16px;
  border-radius: 14px;
  background: var(--surface-card-muted);
  border: 1px solid var(--stroke-soft);
}

.meta-label {
  color: var(--color-text-secondary);
  font-size: 13px;
}

.meta-item strong {
  color: var(--color-text-heading);
  font-size: 14px;
}

@media (max-width: 768px) {
  .meta-item {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
