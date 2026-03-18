import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { ElMessage } from "element-plus";
import type { User } from "@/types";

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
    user.value = {
      id: 1,
      username: "学习者",
      email: "",
      created_at: "",
    };
    initialized.value = true;
    return true;
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
