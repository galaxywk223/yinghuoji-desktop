<template>
  <div class="focus-view">
    <PageContainer
      :title="{ icon: 'lucide:timer-reset', text: '专注计时' }"
      subtitle="专心做一件事，把每一段投入都记下来"
      header-variant="hero"
      max-width="full"
      fill-height
    >
      <div class="focus-layout">
        <div class="focus-layout__timer">
          <!-- 计时器显示 -->
          <FocusTimer
            :elapsed-seconds="elapsedSeconds"
            :display-seconds="presentedDisplaySeconds"
            :target-duration-seconds="presentedTargetDurationSeconds"
            :countdown-progress="presentedCountdownProgress"
            :timer-mode="presentedTimerMode"
            :status="focusStatus"
            :is-active="isTimerRunning"
          />

          <!-- 控制按钮 -->
          <FocusControls
            :is-running="isTimerRunning"
            :is-paused="isPaused"
            :is-completed="isCompleted"
            :timer-mode="presentedTimerMode"
            :loading="loading"
            @start="startTimer"
            @pause="pauseTimer"
            @resume="resumeTimer"
            @stop="showStopDialog"
            @restart="restartTimer"
            @cancel="cancelSession"
            @go-back="goBack"
            @review="stopDialogVisible = true"
          />
        </div>

        <div class="focus-layout__details">
          <div class="focus-panel-head">
            <div>
              <p class="panel-eyebrow">本次记录</p>
              <h3>{{ focusStatus === "idle" ? "开始前确认内容" : "专注信息" }}</h3>
            </div>
            <span class="stage-chip">{{ activeStageLabel }}</span>
          </div>

          <!-- 表单区域 -->
          <FocusForm
            v-if="focusStatus === 'idle'"
            ref="formRef"
            v-model:form-data="focusForm"
            :categories="categories"
            :subcategories="allSubcategories"
            @category-change="onCategoryChange"
          />

          <!-- 已开始时显示的信息 -->
          <FocusInfo
            v-else
            :form-data="focusForm"
            :categories="categories"
            :subcategories="allSubcategories"
          />
        </div>
      </div>
      <!-- 结束专注弹窗 -->
      <el-dialog
        v-model="stopDialogVisible"
        :show-close="false"
        :close-on-click-modal="!isAutomaticCompletion"
        :close-on-press-escape="!isAutomaticCompletion"
        :width="isAutomaticCompletion ? '380px' : '320px'"
        class="ios-dialog-modal"
        :class="{ 'completion-dialog-modal': isAutomaticCompletion }"
        align-center
        destroy-on-close
      >
        <div class="ios-dialog-content">
          <div class="ios-dialog-header" :class="{ completed: isAutomaticCompletion }">
            <span v-if="isAutomaticCompletion" class="completion-icon" aria-hidden="true">
              <Icon :icon="bellRingIcon" />
            </span>
            <h3 class="ios-dialog-title">
              {{ isAutomaticCompletion ? "倒计时结束" : "保存学习记录" }}
            </h3>
            <p class="ios-dialog-subtitle">
              {{ isAutomaticCompletion ? "本轮专注已完成，请处理本次记录" : "本次专注已结束" }}
            </p>
          </div>

          <div class="ios-summary-card">
            <div class="summary-row">
              <div class="summary-item">
                <span class="label">时长</span>
                <span class="value highlight">{{
                  formatDuration(elapsedSeconds)
                }}</span>
              </div>
              <div class="divider-vertical"></div>
              <div class="summary-item">
                <span class="label">时间段</span>
                <span class="value"
                  >{{ startTimeDisplay }} - {{ endTimeDisplay }}</span
                >
              </div>
            </div>
          </div>

          <div class="ios-form-group">
            <div class="form-row">
              <span class="row-label">心情</span>
              <el-rate
                v-model="stopForm.mood"
                :colors="[
                  'var(--color-text-muted)',
                  'var(--color-warning)',
                  'var(--color-warning)',
                ]"
                size="large"
                class="ios-rate"
              />
            </div>
            <div class="form-row column">
              <textarea
                v-model="stopForm.notes"
                class="ios-textarea"
                rows="3"
                placeholder="写点什么..."
                maxlength="200"
              ></textarea>
            </div>
          </div>

          <div class="form-footer">
            <div
              v-if="isAutomaticCompletion"
              class="completion-actions"
            >
              <button
                class="pill-btn primary"
                :disabled="loading"
                @click="saveRecord"
              >
                保存记录
              </button>
              <button
                class="pill-btn secondary"
                :disabled="loading"
                @click="restartCompletedCountdown"
              >
                重新开始
              </button>
              <button
                class="completion-discard"
                :disabled="loading"
                @click="discardCompletedCountdown"
              >
                放弃记录
              </button>
            </div>
            <div v-else class="pill-btn-group-horizontal">
              <button
                class="pill-btn secondary"
                @click="stopDialogVisible = false"
              >
                取消
              </button>
              <button
                class="pill-btn primary"
                :disabled="loading"
                @click="saveRecord"
              >
                保存
              </button>
            </div>
          </div>
        </div>
      </el-dialog>
    </PageContainer>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onActivated, watch } from "vue";
import { useRouter } from "vue-router";
import { ElMessage, ElMessageBox } from "element-plus";
import { Icon } from "@iconify/vue";
import bellRingIcon from "@iconify-icons/lucide/bell-ring";
import { useCategoryStore } from "@/stores/category";
import { useStageStore } from "@/stores/modules/stage";
import { useSettingsStore } from "@/stores/modules/settings";
import { recordApi } from "@/api/modules/records";
import { useFocusTimer } from "@/composables/useFocusTimer";
import { getFocusLogDate } from "@/utils/focusLearningDay";
import {
  primeFocusAlert,
  startFocusAlertLoop,
  stopFocusAlert,
} from "@/utils/focusAlert";

// 组件导入
import FocusTimer from "@/components/business/focus/FocusTimer.vue";
import FocusForm from "@/components/business/focus/FocusForm.vue";
import FocusInfo from "@/components/business/focus/FocusInfo.vue";
import FocusControls from "@/components/business/focus/FocusControls.vue";
import PageContainer from "@/components/layout/PageContainer.vue";

const router = useRouter();
const categoryStore = useCategoryStore();
const stageStore = useStageStore();
const settingsStore = useSettingsStore();

// 使用计时器 composable
const {
  status: focusStatus,
  isTimerRunning,
  isPaused,
  isCompleted,
  timerMode,
  elapsedSeconds,
  displaySeconds,
  targetDurationSeconds,
  countdownProgress,
  completionReason,
  sessionStartTime,
  sessionEndTime,
  startTimer: timerStart,
  pauseTimer: timerPause,
  resumeTimer: timerResume,
  stopTimer: timerStop,
  restartTimer: timerRestart,
  cancelSession: timerCancel,
  restoreState,
  clearState,
} = useFocusTimer();

// 表单数据
const focusForm = ref({
  name: "",
  categoryId: null,
  subcategoryId: null,
  mode: "countup",
  durationMinutes: 30,
});

// 结束弹窗数据
const stopDialogVisible = ref(false);
const stopForm = ref({
  mood: 0,
  notes: "",
});

const formRef = ref(null);
const loading = ref(false);
const isAutomaticCompletion = computed(
  () => isCompleted.value && completionReason.value === "countdown",
);
const presentedTimerMode = computed(() =>
  focusStatus.value === "idle" ? focusForm.value.mode : timerMode.value,
);
const presentedTargetDurationSeconds = computed(() =>
  focusStatus.value === "idle" && presentedTimerMode.value === "countdown"
    ? focusForm.value.durationMinutes * 60
    : targetDurationSeconds.value,
);
const presentedDisplaySeconds = computed(() =>
  focusStatus.value === "idle" && presentedTimerMode.value === "countdown"
    ? presentedTargetDurationSeconds.value
    : displaySeconds.value,
);
const presentedCountdownProgress = computed(() =>
  focusStatus.value === "idle" && presentedTimerMode.value === "countdown"
    ? 1
    : countdownProgress.value,
);

watch(
  isAutomaticCompletion,
  (completed) => {
    if (completed) stopDialogVisible.value = true;
  },
  { immediate: true },
);

// 分类和子分类数据
const categories = computed(() => categoryStore.tree || []);
const allSubcategories = computed(() =>
  categories.value.flatMap((category) => {
    const subs = category.subcategories || category.children || [];
    return subs.map((sub) => ({
      id: sub.id,
      name: sub.name,
      category_id: sub.category_id || category.id,
      color: sub.color,
    }));
  }),
);

// 格式化时间显示
const startTimeDisplay = computed(() => {
  if (!sessionStartTime.value) return "--";
  return new Date(sessionStartTime.value).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  });
});

const endTimeDisplay = computed(() => {
  if (!sessionEndTime.value) return "--";
  return new Date(sessionEndTime.value).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  });
});

// 格式化时长
const formatDuration = (seconds) => {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  } else if (minutes > 0) {
    return `${minutes}m ${secs}s`;
  } else {
    return `${secs}s`;
  }
};

const activeStageLabel = computed(
  () => stageStore.activeStage?.name || "未选择阶段",
);

const syncSharedData = async () => {
  await stageStore.fetchStages();

  if (!stageStore.activeStage) {
    ElMessage.warning("请先在学习阶段中创建并启用当前阶段");
    return false;
  }

  await categoryStore.fetchCategories();
  return true;
};

// 加载数据
const loadData = async () => {
  try {
    const ready = await syncSharedData();
    if (!ready) return;
  } catch (error) {
    console.error("加载数据失败:", error);
    ElMessage.error("加载数据失败");
  }
};

// 分类切换时重置子分类
const onCategoryChange = () => {
  focusForm.value.subcategoryId = null;
};

// 开始计时
const startTimer = async () => {
  try {
    await formRef.value?.validate();
    stopFocusAlert();
    if (focusForm.value.mode === "countdown") {
      await primeFocusAlert().catch(() => void 0);
    }
    await timerStart(focusForm.value, {
      mode: focusForm.value.mode,
      durationMinutes: focusForm.value.durationMinutes,
    });
    ElMessage.success(
      focusForm.value.mode === "countdown"
        ? `已开始 ${focusForm.value.durationMinutes} 分钟倒计时`
        : "开始专注！保持专注，加油！",
    );
  } catch (error) {
    console.error("表单验证失败:", error);
  }
};

// 暂停计时
const pauseTimer = async () => {
  await timerPause(focusForm.value);
  ElMessage.info("已暂停");
};

// 继续计时
const resumeTimer = async () => {
  await timerResume(focusForm.value);
  ElMessage.success("继续专注！");
};

const formatElapsedClock = (secondsValue) => {
  const hours = Math.floor(secondsValue / 3600);
  const minutes = Math.floor((secondsValue % 3600) / 60);
  const seconds = secondsValue % 60;

  return hours > 0
    ? `${hours}:${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`
    : `${minutes}:${seconds.toString().padStart(2, "0")}`;
};

// 重新开始当前专注
const restartTimer = async () => {
  try {
    await ElMessageBox.confirm(
      `确认重新开始当前专注？已专注 ${formatElapsedClock(elapsedSeconds.value)}，当前计时将被清零且不会保存。`,
      "重新开始",
      {
        confirmButtonText: "确认重新开始",
        cancelButtonText: "取消",
        type: "warning",
      },
    );

    stopDialogVisible.value = false;
    stopFocusAlert();
    await timerRestart(focusForm.value);
    ElMessage.success("已重新开始专注");
  } catch {
    // 用户取消操作
  }
};

// 显示停止确认弹窗
const showStopDialog = async () => {
  await timerStop();
  stopDialogVisible.value = true;
};

// 保存学习记录
const saveRecord = async () => {
  if (isAutomaticCompletion.value) stopFocusAlert();
  try {
    loading.value = true;

    // 检查是否有激活的阶段
    if (!stageStore.activeStage) {
      ElMessage.error("请先在学习阶段中创建并启用当前阶段");
      loading.value = false;
      return;
    }

    // 计算持续时间（分钟）
    const durationMinutes = Math.ceil(elapsedSeconds.value / 60);

    // 格式化时间段
    const timeSlot = `${startTimeDisplay.value}-${endTimeDisplay.value}`;
    const logDate = getFocusLogDate(
      sessionEndTime.value ?? new Date(),
      settingsStore.focusDayBoundaryHour,
    );

    // 保存学习记录
    const recordData = {
      stage_id: stageStore.activeStage.id,
      task: focusForm.value.name,
      subcategory_id: focusForm.value.subcategoryId,
      actual_duration: durationMinutes,
      log_date: logDate,
      time_slot: timeSlot,
      mood: stopForm.value.mood,
      notes: stopForm.value.notes || "",
    };

    await recordApi.createRecord(recordData);

    stopDialogVisible.value = false;
    stopForm.value = {
      mood: 3,
      notes: "",
    };
    clearState();

    ElMessage.success("专注记录已保存！");

    loading.value = false;
    setTimeout(() => {
      router.push("/records");
    }, 1500);
  } catch (error) {
    console.error("保存记录失败:", error);

    let errorMessage = "保存记录失败";
    if (error.response) {
      errorMessage =
        error.response.data?.message || `服务器错误: ${error.response.status}`;
    } else if (error.request) {
      errorMessage = "网络连接失败，请检查网络或后端服务";
    } else {
      errorMessage = error.message || "未知错误";
    }

    ElMessage.error(errorMessage);
    loading.value = false;
  }
};

// 放弃当前专注会话
const cancelSession = async () => {
  try {
    await ElMessageBox.confirm(
      `确认放弃当前专注记录？已专注 ${formatElapsedClock(elapsedSeconds.value)}，数据将不会保存。`,
      "放弃记录",
      {
        confirmButtonText: "确认放弃",
        cancelButtonText: "取消",
        type: "warning",
      },
    );

    stopFocusAlert();
    await timerCancel();

    // 重置表单
    focusForm.value = {
      name: "",
      categoryId: null,
      subcategoryId: null,
      notes: "",
      mode: "countup",
      durationMinutes: 30,
    };

    ElMessage.info("已放弃专注记录");
  } catch {
    // 用户取消操作
  }
};

const discardCompletedCountdown = async () => {
  try {
    await ElMessageBox.confirm(
      "确认放弃这次已完成的专注记录？数据将不会保存。",
      "放弃记录",
      {
        confirmButtonText: "确认放弃",
        cancelButtonText: "返回",
        type: "warning",
      },
    );
  } catch {
    await startFocusAlertLoop();
    return;
  }

  stopFocusAlert();
  await timerCancel();
  stopDialogVisible.value = false;
  focusForm.value = {
    name: "",
    categoryId: null,
    subcategoryId: null,
    mode: "countup",
    durationMinutes: 30,
  };
  ElMessage.info("已放弃专注记录");
};

const restartCompletedCountdown = async () => {
  stopFocusAlert();
  stopDialogVisible.value = false;
  await timerRestart(focusForm.value);
  ElMessage.success(`已重新开始 ${focusForm.value.durationMinutes} 分钟倒计时`);
};

// 返回
const goBack = () => {
  router.back();
};

// 生命周期
onMounted(async () => {
  await loadData();

  const savedFormData = restoreState();
  if (savedFormData) {
    focusForm.value = savedFormData;
    ElMessage.success("已恢复上次的专注记录");
  }
});

onActivated(() => {
  // 每次进入页面时重置临时 UI 状态，并同步共享数据
  loading.value = false;
  stopDialogVisible.value = isCompleted.value;
  void syncSharedData();
});
</script>

<style scoped lang="scss">
.focus-view {
  position: relative;
  min-height: 100%;
}

.focus-layout {
  position: relative;
  z-index: 1;
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(300px, 380px) minmax(0, 1fr);
  gap: 14px;
  align-items: start;
  margin-top: 4px;

  &__timer {
    display: flex;
    flex-direction: column;
    gap: 12px;
    align-items: center;
    padding: 20px 18px 16px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    box-shadow: var(--shadow-1);
  }

  &__details {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 100%;
    padding: 16px 18px 18px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    box-shadow: var(--shadow-1);
  }
}

.focus-panel-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding-bottom: 4px;
  border-bottom: 1px solid var(--border-subtle);
}

.panel-eyebrow {
  margin: 0 0 4px;
  color: var(--text-muted);
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.focus-panel-head h3 {
  margin: 0;
  color: var(--text-primary);
  font-size: 1.02rem;
  line-height: 1.3;
  font-weight: 700;
}

.stage-chip {
  display: inline-flex;
  align-items: center;
  min-height: 28px;
  max-width: 220px;
  padding: 0 10px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-subtle);
  background: var(--bg-muted);
  color: var(--text-secondary);
  font-size: 0.8rem;
  font-weight: 700;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.focus-layout :deep(.focus-controls) {
  margin-top: 0;
}

.focus-layout :deep(.button-stack) {
  max-width: 100%;
}

/* iOS Dialog Styles */
:deep(.ios-dialog-modal) {
  .el-dialog {
    background: var(--surface-card);
    border: 1px solid var(--stroke-soft);
    border-radius: 14px;
    box-shadow: var(--box-shadow-hover);
    padding: 0;
    overflow: hidden;

    .el-dialog__header {
      display: none; /* Hide default header */
    }

    .el-dialog__body {
      padding: 0;
    }
  }
}

.ios-dialog-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
}

.ios-dialog-header {
  padding: 24px 16px 16px;

  .ios-dialog-title {
    font-size: 17px;
    font-weight: 600;
    color: var(--color-text-heading);
    margin: 0 0 4px;
    line-height: 1.3;
  }

  .ios-dialog-subtitle {
    font-size: 13px;
    color: var(--color-text-secondary);
    margin: 0;
  }

  &.completed {
    width: 100%;
    padding-top: 28px;
    background: color-mix(in srgb, var(--color-success) 10%, var(--surface-card));
    border-bottom: 1px solid color-mix(in srgb, var(--color-success) 25%, transparent);

    .ios-dialog-title {
      color: var(--color-success);
      font-size: 20px;
      font-weight: 800;
    }
  }
}

.completion-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  margin-bottom: 12px;
  border-radius: 50%;
  background: var(--color-success);
  color: var(--color-text-inverse);

  svg {
    width: 24px;
    height: 24px;
  }
}

.ios-summary-card {
  width: 100%;
  padding: 0 16px;
  margin-bottom: 20px;

  .summary-row {
    background: var(--surface-card-muted);
    border-radius: 10px;
    padding: 12px;
    display: flex;
    justify-content: space-around;
    align-items: center;
  }

  .summary-item {
    display: flex;
    flex-direction: column;
    gap: 2px;

    .label {
      font-size: 11px;
      color: var(--color-text-secondary);
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .value {
      font-size: 15px;
      font-weight: 500;
      color: var(--color-text-heading);

      &.highlight {
        color: var(--color-primary);
        font-weight: 600;
      }
    }
  }

  .divider-vertical {
    width: 1px;
    height: 24px;
    background: var(--stroke-soft);
  }
}

.ios-form-group {
  width: 100%;
  padding: 0 16px;
  margin-bottom: 24px;
  display: flex;
  flex-direction: column;
  gap: 12px;

  .form-row {
    display: flex;
    align-items: center;
    justify-content: space-between;

    &.column {
      flex-direction: column;
      align-items: stretch;
    }

    .row-label {
      font-size: 15px;
      color: var(--color-text-heading);
    }
  }
}

.ios-textarea {
  width: 100%;
  background: var(--surface-card-muted);
  border: 1px solid var(--color-border-input);
  border-radius: 8px;
  padding: 8px 12px;
  font-size: 15px;
  color: var(--color-text-heading);
  resize: none;
  outline: none;
  font-family: inherit;

  &::placeholder {
    color: var(--color-text-muted);
  }

  &:focus {
    background: var(--surface-card);
    border-color: var(--color-primary);
  }
}

.form-footer {
  width: 100%;
  padding: 16px 24px 24px;
  background: var(--surface-card);
  border-top: 1px solid var(--stroke-soft);
  margin-top: auto;
}

.completion-actions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;

  .pill-btn {
    width: 100%;
    min-height: 42px;
  }
}

.completion-discard {
  grid-column: 1 / -1;
  border: 0;
  background: transparent;
  color: var(--color-error);
  min-height: 36px;
  font: inherit;
  font-size: 0.9rem;
  font-weight: 650;
  cursor: pointer;

  &:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }
}

@media (max-width: 960px) {
  .focus-layout {
    grid-template-columns: 1fr;
  }
}
</style>
