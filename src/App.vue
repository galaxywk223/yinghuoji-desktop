<template>
  <el-config-provider :locale="zhCn" size="default" :z-index="3000">
    <div id="app">
      <router-view v-slot="{ Component }">
        <keep-alive :max="5">
          <component :is="Component" :key="$route.path" />
        </keep-alive>
      </router-view>
    </div>
  </el-config-provider>
</template>

<script setup lang="ts">
import { onMounted } from "vue";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import { useAuthStore } from "@/stores/modules/auth";
import { useSettingsStore } from "@/stores/modules/settings";
import { useThemeStore } from "@/stores/modules/theme";
import { scheduleStartupUpdateCheck } from "@/utils/updater";

const authStore = useAuthStore();
const settingsStore = useSettingsStore();
const themeStore = useThemeStore();

onMounted(() => {
  themeStore.initTheme();
  void authStore.checkAuth();
  void settingsStore.fetchSettings();
  scheduleStartupUpdateCheck();
});
</script>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html,
body {
  height: 100%;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

#app {
  min-height: 100vh;
}

/* 性能优化：减少动画计算（用户系统级偏好） */
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
</style>
