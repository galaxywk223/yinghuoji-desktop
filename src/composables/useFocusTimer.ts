/**
 * 专注计时器 composable
 * 处理计时器的逻辑和状态管理
 */
import { ref, type Ref } from "vue";
import type { FocusFormData } from "@/types";

const FOCUS_STATE_KEY = "focus_session_state";

// 存储的状态类型
interface SavedState {
  formData: FocusFormData;
  isTimerRunning: boolean;
  isPaused: boolean;
  elapsedSeconds: number;
  sessionStartTime?: string | null;
  sessionEndTime?: string | null;
  pauseStartedAt?: string | null;
  currentRunStartedAt?: string | null;
  accumulatedElapsedSeconds?: number;
  startTime?: string | null;
  pauseTime?: string | null;
}

// 返回值类型
interface UseFocusTimerReturn {
  // 状态
  isTimerRunning: Ref<boolean>;
  isPaused: Ref<boolean>;
  elapsedSeconds: Ref<number>;
  sessionStartTime: Ref<Date | null>;
  sessionEndTime: Ref<Date | null>;
  pauseStartedAt: Ref<Date | null>;

  // 方法
  startTimer: (formData: FocusFormData) => void;
  pauseTimer: (formData: FocusFormData) => void;
  resumeTimer: (formData: FocusFormData) => void;
  stopTimer: () => number;
  cancelSession: () => void;
  completeSession: () => number;
  restoreState: () => FocusFormData | null;
  clearState: () => void;
  resetTimer: () => void;
  saveState: (formData: FocusFormData) => void;
}

// 全局单例状态（模块级变量）
const isTimerRunning: Ref<boolean> = ref(false);
const isPaused: Ref<boolean> = ref(false);
const elapsedSeconds: Ref<number> = ref(0);
const sessionStartTime: Ref<Date | null> = ref(null);
const sessionEndTime: Ref<Date | null> = ref(null);
const pauseStartedAt: Ref<Date | null> = ref(null);
const currentRunStartedAt: Ref<Date | null> = ref(null);
const accumulatedElapsedSeconds: Ref<number> = ref(0);
const timerInterval: Ref<NodeJS.Timeout | number | null> = ref(null);

export function useFocusTimer(): UseFocusTimerReturn {
  const syncElapsedSeconds = (now = new Date()): number => {
    if (!isTimerRunning.value || !currentRunStartedAt.value) {
      elapsedSeconds.value = accumulatedElapsedSeconds.value;
      return elapsedSeconds.value;
    }

    const diffSeconds = Math.max(
      0,
      Math.floor((now.getTime() - currentRunStartedAt.value.getTime()) / 1000),
    );
    elapsedSeconds.value = accumulatedElapsedSeconds.value + diffSeconds;
    return elapsedSeconds.value;
  };

  // 计时器状态（已移至全局）到 localStorage
  const saveState = (formData: FocusFormData): void => {
    const state: SavedState = {
      formData,
      isTimerRunning: isTimerRunning.value,
      isPaused: isPaused.value,
      elapsedSeconds: elapsedSeconds.value,
      sessionStartTime: sessionStartTime.value
        ? sessionStartTime.value.toISOString()
        : null,
      sessionEndTime: sessionEndTime.value
        ? sessionEndTime.value.toISOString()
        : null,
      pauseStartedAt: pauseStartedAt.value
        ? pauseStartedAt.value.toISOString()
        : null,
      currentRunStartedAt: currentRunStartedAt.value
        ? currentRunStartedAt.value.toISOString()
        : null,
      accumulatedElapsedSeconds: accumulatedElapsedSeconds.value,
    };
    localStorage.setItem(FOCUS_STATE_KEY, JSON.stringify(state));
  };

  // 开始计时器间隔
  const startTimerInterval = (): void => {
    if (timerInterval.value) {
      clearInterval(timerInterval.value);
    }

    timerInterval.value = setInterval(() => {
      if (!currentRunStartedAt.value) {
        return;
      }
      syncElapsedSeconds();
    }, 1000);
  };

  // 停止计时器间隔
  const stopTimerInterval = (): void => {
    if (timerInterval.value) {
      clearInterval(timerInterval.value);
      timerInterval.value = null;
    }
  };

  // 从 localStorage 恢复状态
  const restoreState = (): FocusFormData | null => {
    try {
      const savedState = localStorage.getItem(FOCUS_STATE_KEY);
      if (savedState) {
        const state: SavedState = JSON.parse(savedState);
        console.log("恢复的专注状态:", state);

        isTimerRunning.value = state.isTimerRunning || false;
        isPaused.value = state.isPaused || false;
        accumulatedElapsedSeconds.value =
          state.accumulatedElapsedSeconds ?? state.elapsedSeconds ?? 0;
        elapsedSeconds.value = state.elapsedSeconds || 0;

        const resolvedSessionStart =
          state.sessionStartTime ?? state.startTime ?? null;
        const resolvedPauseStartedAt =
          state.pauseStartedAt ?? state.pauseTime ?? null;

        sessionStartTime.value = resolvedSessionStart
          ? new Date(resolvedSessionStart)
          : null;
        sessionEndTime.value = state.sessionEndTime
          ? new Date(state.sessionEndTime)
          : null;
        pauseStartedAt.value = resolvedPauseStartedAt
          ? new Date(resolvedPauseStartedAt)
          : null;
        currentRunStartedAt.value = state.currentRunStartedAt
          ? new Date(state.currentRunStartedAt)
          : isTimerRunning.value && state.startTime
            ? new Date(state.startTime)
            : null;

        if (isTimerRunning.value) {
          syncElapsedSeconds();
          if (!timerInterval.value) {
            startTimerInterval();
          }
        } else {
          elapsedSeconds.value = accumulatedElapsedSeconds.value;
        }

        return state.formData;
      }
    } catch (error) {
      console.error("恢复专注状态失败:", error);
      clearState();
    }
    return null;
  };

  // 清除状态
  const clearState = (): void => {
    localStorage.removeItem(FOCUS_STATE_KEY);
    resetTimer();
  };

  // 重置计时器
  const resetTimer = (): void => {
    isTimerRunning.value = false;
    isPaused.value = false;
    elapsedSeconds.value = 0;
    sessionStartTime.value = null;
    sessionEndTime.value = null;
    pauseStartedAt.value = null;
    currentRunStartedAt.value = null;
    accumulatedElapsedSeconds.value = 0;
    stopTimerInterval();
  };

  // 开始计时
  const startTimer = (formData: FocusFormData): void => {
    const now = new Date();
    sessionStartTime.value = now;
    sessionEndTime.value = null;
    pauseStartedAt.value = null;
    currentRunStartedAt.value = now;
    accumulatedElapsedSeconds.value = 0;
    isTimerRunning.value = true;
    isPaused.value = false;
    elapsedSeconds.value = 0;

    startTimerInterval();
    saveState(formData);

    console.log("开始专注计时:", now);
  };

  // 暂停计时
  const pauseTimer = (formData: FocusFormData): void => {
    if (isTimerRunning.value) {
      syncElapsedSeconds();
      accumulatedElapsedSeconds.value = elapsedSeconds.value;
      pauseStartedAt.value = new Date();
      currentRunStartedAt.value = null;
      isTimerRunning.value = false;
      isPaused.value = true;

      stopTimerInterval();
      saveState(formData);

      console.log("暂停专注计时:", pauseStartedAt.value);
    }
  };

  // 恢复计时
  const resumeTimer = (formData: FocusFormData): void => {
    if (isPaused.value && pauseStartedAt.value && sessionStartTime.value) {
      currentRunStartedAt.value = new Date();
      isTimerRunning.value = true;
      isPaused.value = false;
      pauseStartedAt.value = null;
      sessionEndTime.value = null;

      startTimerInterval();
      saveState(formData);

      console.log("恢复专注计时");
    }
  };

  // 停止计时
  const stopTimer = (): number => {
    if (isTimerRunning.value) {
      syncElapsedSeconds();
      accumulatedElapsedSeconds.value = elapsedSeconds.value;
      sessionEndTime.value = new Date();
      currentRunStartedAt.value = null;
    } else if (isPaused.value) {
      elapsedSeconds.value = accumulatedElapsedSeconds.value;
      sessionEndTime.value = pauseStartedAt.value
        ? new Date(pauseStartedAt.value)
        : new Date();
    }

    const finalElapsed = elapsedSeconds.value;
    stopTimerInterval();
    return finalElapsed;
  };

  // 取消会话
  const cancelSession = (): void => {
    clearState();
    console.log("已取消专注会话");
  };

  // 完成会话
  const completeSession = (): number => {
    const finalElapsed = elapsedSeconds.value;
    clearState();
    return finalElapsed;
  };

  // 注意：不再使用 onUnmounted 清理计时器，以支持后台计时（路由切换时）

  return {
    // 状态
    isTimerRunning,
    isPaused,
    elapsedSeconds,
    sessionStartTime,
    sessionEndTime,
    pauseStartedAt,

    // 方法
    startTimer,
    pauseTimer,
    resumeTimer,
    stopTimer,
    cancelSession,
    completeSession,
    restoreState,
    clearState,
    resetTimer,
    saveState,
  };
}
