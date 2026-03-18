/**
 * 本地档案状态管理
 */
import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { authAPI } from "@/api";
import { ElMessage } from "element-plus";
import type {
  User,
  UserProfileResponse,
} from "@/types";

export const useAuthStore = defineStore("auth", () => {
  const user = ref<User | null>(null);
  const accessToken = ref("local-mode-token");
  const refreshToken = ref("");
  const initialized = ref(false);

  const isAuthenticated = computed(() => initialized.value);

  async function login(credentials: any) {
    void credentials;
    ElMessage.warning("桌面端为单用户本地模式，不支持登录。");
    return false;
  }

  async function register(userInfo: any) {
    void userInfo;
    ElMessage.warning("桌面端为单用户本地模式，不支持注册。");
    return false;
  }

  function logout() {
    ElMessage.info("桌面端当前为本地单用户模式，无需退出登录。");
  }

  async function checkAuth() {
    try {
      const response =
        (await authAPI.getCurrentUser()) as unknown as UserProfileResponse;
      if (response.success) {
        user.value = response.user || (response as any).data || null;
        initialized.value = true;
        return true;
      }
      initialized.value = true;
      return false;
    } catch (error) {
      console.error("加载本地档案失败:", error);
      initialized.value = true;
      return false;
    }
  }

  async function refresh() {
    return true;
  }

  return {
    user,
    accessToken,
    refreshToken,
    initialized,
    isAuthenticated,
    login,
    register,
    logout,
    checkAuth,
    refresh,
  };
});
