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
import { onBeforeUnmount, onMounted, watch } from "vue";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/stores/modules/auth";
import { useSettingsStore } from "@/stores/modules/settings";
import { useThemeStore } from "@/stores/modules/theme";
import { useFocusTimer } from "@/composables/useFocusTimer";
import { startFocusAlertLoop, stopFocusAlert } from "@/utils/focusAlert";
import { ensureFocusCompletionNotification } from "@/utils/focusReminder";
import { scheduleStartupUpdateCheck } from "@/utils/updater";

const router = useRouter();
const authStore = useAuthStore();
const settingsStore = useSettingsStore();
const themeStore = useThemeStore();
const {
  status: focusStatus,
  completionReason,
  sessionFormData,
  targetDurationSeconds,
  completionNotificationSent,
  saveState: saveFocusState,
  forceCompleteCountdown,
  restoreState: restoreFocusState,
} = useFocusTimer();
let unlistenFocusCompletion: UnlistenFn | null = null;

watch(
  [focusStatus, completionReason],
  async ([nextStatus, reason]) => {
    if (nextStatus !== "completed" || reason !== "countdown") {
      if (nextStatus !== "completed") stopFocusAlert();
      return;
    }

    if (!completionNotificationSent.value) {
      completionNotificationSent.value = await ensureFocusCompletionNotification(
        sessionFormData.value?.name || "本次专注",
        targetDurationSeconds.value / 60,
      );
      saveFocusState();
    }
    await startFocusAlertLoop();

    if (isTauri()) {
      await invoke("app_restore_main_window").catch((error) => {
        console.error("恢复专注窗口失败", error);
      });
    }
    if (router.currentRoute.value.path !== "/focus") {
      await router.push("/focus");
    }
  },
  { immediate: true },
);

onMounted(async () => {
  themeStore.initTheme();
  void authStore.checkAuth();
  void settingsStore.fetchSettings();
  scheduleStartupUpdateCheck();
  restoreFocusState();

  if (isTauri()) {
    unlistenFocusCompletion = await listen("focus-countdown-completed", () => {
      forceCompleteCountdown();
    });
  }
});

onBeforeUnmount(() => {
  unlistenFocusCompletion?.();
  stopFocusAlert();
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
