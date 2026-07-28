import { computed, ref, type ComputedRef, type Ref } from "vue";
import type { FocusFormData, FocusTimerMode } from "@/types";
import {
  cancelFocusReminder,
  scheduleFocusReminder,
} from "@/utils/focusReminder";

const FOCUS_STATE_KEY = "focus_session_state";
const DEFAULT_COUNTDOWN_MINUTES = 30;

export type FocusSessionStatus = "idle" | "running" | "paused" | "completed";
export type FocusCompletionReason = "manual" | "countdown" | null;

interface SavedState {
  version?: number;
  formData: FocusFormData;
  status?: FocusSessionStatus;
  mode?: FocusTimerMode;
  targetDurationSeconds?: number;
  completionReason?: FocusCompletionReason;
  completionNotificationSent?: boolean;
  isTimerRunning?: boolean;
  isPaused?: boolean;
  elapsedSeconds?: number;
  sessionStartTime?: string | null;
  sessionEndTime?: string | null;
  pauseStartedAt?: string | null;
  currentRunStartedAt?: string | null;
  accumulatedElapsedSeconds?: number;
  startTime?: string | null;
  pauseTime?: string | null;
}

interface FocusTimerConfig {
  mode?: FocusTimerMode;
  durationMinutes?: number;
}

interface UseFocusTimerReturn {
  status: Ref<FocusSessionStatus>;
  isTimerRunning: ComputedRef<boolean>;
  isPaused: ComputedRef<boolean>;
  isCompleted: ComputedRef<boolean>;
  timerMode: Ref<FocusTimerMode>;
  elapsedSeconds: Ref<number>;
  displaySeconds: ComputedRef<number>;
  targetDurationSeconds: Ref<number>;
  countdownProgress: ComputedRef<number>;
  completionReason: Ref<FocusCompletionReason>;
  completionNotificationSent: Ref<boolean>;
  sessionFormData: Ref<FocusFormData | null>;
  sessionStartTime: Ref<Date | null>;
  sessionEndTime: Ref<Date | null>;
  pauseStartedAt: Ref<Date | null>;
  startTimer: (formData: FocusFormData, config?: FocusTimerConfig) => Promise<void>;
  pauseTimer: (formData: FocusFormData) => Promise<void>;
  resumeTimer: (formData: FocusFormData) => Promise<void>;
  stopTimer: () => Promise<number>;
  restartTimer: (formData?: FocusFormData) => Promise<void>;
  forceCompleteCountdown: () => void;
  cancelSession: () => Promise<void>;
  completeSession: () => number;
  restoreState: () => FocusFormData | null;
  clearState: () => void;
  resetTimer: () => void;
  saveState: (formData?: FocusFormData | null) => void;
}

const status = ref<FocusSessionStatus>("idle");
const timerMode = ref<FocusTimerMode>("countup");
const elapsedSeconds = ref(0);
const targetDurationSeconds = ref(0);
const completionReason = ref<FocusCompletionReason>(null);
const sessionFormData = ref<FocusFormData | null>(null);
const sessionStartTime = ref<Date | null>(null);
const sessionEndTime = ref<Date | null>(null);
const pauseStartedAt = ref<Date | null>(null);
const currentRunStartedAt = ref<Date | null>(null);
const accumulatedElapsedSeconds = ref(0);
const completionNotificationSent = ref(false);
const timerInterval = ref<ReturnType<typeof setInterval> | null>(null);
let stateRestored = false;

const isTimerRunning = computed(() => status.value === "running");
const isPaused = computed(() => status.value === "paused");
const isCompleted = computed(() => status.value === "completed");
const displaySeconds = computed(() =>
  timerMode.value === "countdown"
    ? Math.max(0, targetDurationSeconds.value - elapsedSeconds.value)
    : elapsedSeconds.value,
);
const countdownProgress = computed(() => {
  if (timerMode.value !== "countdown" || targetDurationSeconds.value <= 0) {
    return 0;
  }
  return displaySeconds.value / targetDurationSeconds.value;
});

function validDate(value: string | null | undefined): Date | null {
  if (!value) return null;
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

function normalizedDurationSeconds(config?: FocusTimerConfig): number {
  if (config?.mode !== "countdown") return 0;
  const minutes = Math.min(
    720,
    Math.max(1, Math.round(config.durationMinutes ?? DEFAULT_COUNTDOWN_MINUTES)),
  );
  return minutes * 60;
}

export function useFocusTimer(): UseFocusTimerReturn {
  const stopTimerInterval = (): void => {
    if (timerInterval.value) {
      clearInterval(timerInterval.value);
      timerInterval.value = null;
    }
  };

  const saveState = (formData = sessionFormData.value): void => {
    if (!formData || status.value === "idle") {
      localStorage.removeItem(FOCUS_STATE_KEY);
      return;
    }

    const state: SavedState = {
      version: 2,
      formData,
      status: status.value,
      mode: timerMode.value,
      targetDurationSeconds: targetDurationSeconds.value,
      completionReason: completionReason.value,
      completionNotificationSent: completionNotificationSent.value,
      isTimerRunning: isTimerRunning.value,
      isPaused: isPaused.value,
      elapsedSeconds: elapsedSeconds.value,
      sessionStartTime: sessionStartTime.value?.toISOString() ?? null,
      sessionEndTime: sessionEndTime.value?.toISOString() ?? null,
      pauseStartedAt: pauseStartedAt.value?.toISOString() ?? null,
      currentRunStartedAt: currentRunStartedAt.value?.toISOString() ?? null,
      accumulatedElapsedSeconds: accumulatedElapsedSeconds.value,
    };
    localStorage.setItem(FOCUS_STATE_KEY, JSON.stringify(state));
  };

  const finishCountdown = (now = new Date()): void => {
    if (timerMode.value !== "countdown" || status.value !== "running") return;

    const overshootSeconds = Math.max(
      0,
      elapsedSeconds.value - targetDurationSeconds.value,
    );
    elapsedSeconds.value = targetDurationSeconds.value;
    accumulatedElapsedSeconds.value = targetDurationSeconds.value;
    sessionEndTime.value = new Date(now.getTime() - overshootSeconds * 1000);
    currentRunStartedAt.value = null;
    pauseStartedAt.value = null;
    completionReason.value = "countdown";
    status.value = "completed";
    stopTimerInterval();
    saveState();
  };

  const syncElapsedSeconds = (now = new Date()): number => {
    if (status.value !== "running" || !currentRunStartedAt.value) {
      elapsedSeconds.value = accumulatedElapsedSeconds.value;
      return elapsedSeconds.value;
    }

    const diffSeconds = Math.max(
      0,
      Math.floor((now.getTime() - currentRunStartedAt.value.getTime()) / 1000),
    );
    elapsedSeconds.value = accumulatedElapsedSeconds.value + diffSeconds;

    if (
      timerMode.value === "countdown" &&
      elapsedSeconds.value >= targetDurationSeconds.value
    ) {
      finishCountdown(now);
    }
    return elapsedSeconds.value;
  };

  const startTimerInterval = (): void => {
    stopTimerInterval();
    timerInterval.value = setInterval(() => syncElapsedSeconds(), 250);
  };

  const scheduleCurrentReminder = async (): Promise<void> => {
    if (
      timerMode.value !== "countdown" ||
      status.value !== "running" ||
      !sessionFormData.value
    ) {
      return;
    }

    const remainingSeconds = Math.max(
      0,
      targetDurationSeconds.value - elapsedSeconds.value,
    );
    const deadline = new Date(Date.now() + remainingSeconds * 1000);
    await scheduleFocusReminder(
      deadline,
      sessionFormData.value.name,
      targetDurationSeconds.value / 60,
    );
    saveState();
  };

  const resetTimer = (): void => {
    status.value = "idle";
    timerMode.value = "countup";
    elapsedSeconds.value = 0;
    targetDurationSeconds.value = 0;
    completionReason.value = null;
    sessionFormData.value = null;
    sessionStartTime.value = null;
    sessionEndTime.value = null;
    pauseStartedAt.value = null;
    currentRunStartedAt.value = null;
    accumulatedElapsedSeconds.value = 0;
    completionNotificationSent.value = false;
    stopTimerInterval();
  };

  const clearState = (): void => {
    localStorage.removeItem(FOCUS_STATE_KEY);
    void cancelFocusReminder();
    resetTimer();
  };

  const restoreState = (): FocusFormData | null => {
    if (stateRestored) return sessionFormData.value;
    stateRestored = true;

    try {
      const savedState = localStorage.getItem(FOCUS_STATE_KEY);
      if (!savedState) return null;

      const state: SavedState = JSON.parse(savedState);
      sessionFormData.value = state.formData;
      status.value =
        state.status ??
        (state.isTimerRunning
          ? "running"
          : state.isPaused
            ? "paused"
            : "idle");
      timerMode.value = state.mode ?? state.formData?.mode ?? "countup";
      targetDurationSeconds.value =
        state.targetDurationSeconds ??
        (timerMode.value === "countdown"
          ? (state.formData?.durationMinutes ?? DEFAULT_COUNTDOWN_MINUTES) * 60
          : 0);
      completionReason.value = state.completionReason ?? null;
      completionNotificationSent.value =
        state.completionNotificationSent ?? false;
      accumulatedElapsedSeconds.value =
        state.accumulatedElapsedSeconds ?? state.elapsedSeconds ?? 0;
      elapsedSeconds.value = state.elapsedSeconds ?? 0;
      sessionStartTime.value = validDate(
        state.sessionStartTime ?? state.startTime,
      );
      sessionEndTime.value = validDate(state.sessionEndTime);
      pauseStartedAt.value = validDate(
        state.pauseStartedAt ?? state.pauseTime,
      );
      currentRunStartedAt.value = validDate(state.currentRunStartedAt);

      if (
        status.value === "running" &&
        !currentRunStartedAt.value &&
        state.startTime
      ) {
        currentRunStartedAt.value = validDate(state.startTime);
      }

      if (status.value === "running") {
        syncElapsedSeconds();
        if (status.value === "running") {
          startTimerInterval();
          if (timerMode.value === "countdown") {
            void scheduleCurrentReminder();
          }
        }
      } else {
        elapsedSeconds.value = accumulatedElapsedSeconds.value;
      }
      return sessionFormData.value;
    } catch (error) {
      console.error("恢复专注状态失败", error);
      clearState();
      return null;
    }
  };

  const startTimer = async (
    formData: FocusFormData,
    config: FocusTimerConfig = {},
  ): Promise<void> => {
    const now = new Date();
    sessionFormData.value = { ...formData };
    timerMode.value = config.mode ?? formData.mode ?? "countup";
    targetDurationSeconds.value = normalizedDurationSeconds({
      mode: timerMode.value,
      durationMinutes: config.durationMinutes ?? formData.durationMinutes,
    });
    sessionStartTime.value = now;
    sessionEndTime.value = null;
    pauseStartedAt.value = null;
    currentRunStartedAt.value = now;
    accumulatedElapsedSeconds.value = 0;
    elapsedSeconds.value = 0;
    completionReason.value = null;
    completionNotificationSent.value = false;
    status.value = "running";
    startTimerInterval();
    saveState();

    if (timerMode.value === "countdown") {
      await scheduleCurrentReminder();
    } else {
      await cancelFocusReminder();
    }
  };

  const pauseTimer = async (formData: FocusFormData): Promise<void> => {
    if (status.value !== "running") return;

    syncElapsedSeconds();
    if (status.value !== "running") return;
    accumulatedElapsedSeconds.value = elapsedSeconds.value;
    pauseStartedAt.value = new Date();
    currentRunStartedAt.value = null;
    sessionFormData.value = { ...formData };
    status.value = "paused";
    completionNotificationSent.value = false;
    stopTimerInterval();
    await cancelFocusReminder();
    saveState();
  };

  const resumeTimer = async (formData: FocusFormData): Promise<void> => {
    if (status.value !== "paused" || !sessionStartTime.value) return;

    currentRunStartedAt.value = new Date();
    pauseStartedAt.value = null;
    sessionEndTime.value = null;
    sessionFormData.value = { ...formData };
    status.value = "running";
    startTimerInterval();
    saveState();
    await scheduleCurrentReminder();
  };

  const stopTimer = async (): Promise<number> => {
    if (status.value === "running") {
      syncElapsedSeconds();
      if (isCompleted.value) return elapsedSeconds.value;
      accumulatedElapsedSeconds.value = elapsedSeconds.value;
      sessionEndTime.value = new Date();
    } else if (status.value === "paused") {
      elapsedSeconds.value = accumulatedElapsedSeconds.value;
      sessionEndTime.value = pauseStartedAt.value ?? new Date();
    }

    currentRunStartedAt.value = null;
    pauseStartedAt.value = null;
    completionReason.value = "manual";
    status.value = "completed";
    completionNotificationSent.value = false;
    stopTimerInterval();
    await cancelFocusReminder();
    saveState();
    return elapsedSeconds.value;
  };

  const restartTimer = async (
    formData = sessionFormData.value ?? undefined,
  ): Promise<void> => {
    if (!formData) return;
    const durationMinutes =
      timerMode.value === "countdown"
        ? targetDurationSeconds.value / 60
        : undefined;
    await cancelFocusReminder();
    await startTimer(formData, {
      mode: timerMode.value,
      durationMinutes,
    });
  };

  const forceCompleteCountdown = (): void => {
    if (status.value !== "running" || timerMode.value !== "countdown") return;
    elapsedSeconds.value = targetDurationSeconds.value;
    finishCountdown();
  };

  const cancelSession = async (): Promise<void> => {
    await cancelFocusReminder();
    clearState();
  };

  const completeSession = (): number => {
    const finalElapsed = elapsedSeconds.value;
    clearState();
    return finalElapsed;
  };

  return {
    status,
    isTimerRunning,
    isPaused,
    isCompleted,
    timerMode,
    elapsedSeconds,
    displaySeconds,
    targetDurationSeconds,
    countdownProgress,
    completionReason,
    completionNotificationSent,
    sessionFormData,
    sessionStartTime,
    sessionEndTime,
    pauseStartedAt,
    startTimer,
    pauseTimer,
    resumeTimer,
    stopTimer,
    restartTimer,
    forceCompleteCountdown,
    cancelSession,
    completeSession,
    restoreState,
    clearState,
    resetTimer,
    saveState,
  };
}
