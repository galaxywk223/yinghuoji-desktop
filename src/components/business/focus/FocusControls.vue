<!-- 专注控制按钮组件 -->
<template>
  <div class="focus-controls">
    <template v-if="!isRunning && !isPaused && !isCompleted">
      <div class="button-stack">
        <button
          type="button"
          class="control-btn primary"
          :disabled="loading"
          @click="$emit('start')"
        >
          <el-icon><VideoPlay /></el-icon>
          <span>{{ timerMode === "countdown" ? "开始倒计时" : "开始专注" }}</span>
        </button>
        <button type="button" class="return-link" @click="$emit('go-back')">
          返回
        </button>
      </div>
    </template>

    <template v-else-if="isCompleted">
      <div class="button-stack">
        <button
          type="button"
          class="control-btn primary"
          @click="$emit('review')"
        >
          处理本次记录
        </button>
      </div>
    </template>

    <template v-else-if="isRunning">
      <div class="button-stack">
        <button
          type="button"
          class="control-btn warning"
          @click="$emit('pause')"
        >
          <el-icon><VideoPause /></el-icon>
          <span>暂停</span>
        </button>
        <button
          type="button"
          class="control-btn danger"
          @click="$emit('stop')"
        >
          结束专注
        </button>
        <button
          type="button"
          class="control-btn secondary"
          @click="$emit('restart')"
        >
          <el-icon><RefreshRight /></el-icon>
          <span>重新开始</span>
        </button>
      </div>
    </template>

    <template v-else-if="isPaused">
      <div class="button-stack">
        <button
          type="button"
          class="control-btn primary"
          @click="$emit('resume')"
        >
          <el-icon><VideoPlay /></el-icon>
          <span>继续</span>
        </button>
        <button
          type="button"
          class="control-btn danger"
          @click="$emit('stop')"
        >
          结束专注
        </button>
        <div class="button-row">
          <button
            type="button"
            class="control-btn secondary compact"
            @click="$emit('restart')"
          >
            <el-icon><RefreshRight /></el-icon>
            <span>重新开始</span>
          </button>
          <button
            type="button"
            class="control-btn ghost-danger compact"
            @click="$emit('cancel')"
          >
            放弃记录
          </button>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup>
import { RefreshRight, VideoPlay, VideoPause } from "@element-plus/icons-vue";

defineProps({
  isRunning: {
    type: Boolean,
    default: false,
  },
  isPaused: {
    type: Boolean,
    default: false,
  },
  loading: {
    type: Boolean,
    default: false,
  },
  isCompleted: {
    type: Boolean,
    default: false,
  },
  timerMode: {
    type: String,
    default: "countup",
  },
});

defineEmits([
  "start",
  "pause",
  "resume",
  "stop",
  "restart",
  "cancel",
  "go-back",
  "review",
]);
</script>

<style scoped lang="scss">
.focus-controls {
  width: 100%;
  display: flex;
  justify-content: center;
  margin-top: 4px;
}

.button-stack {
  width: 100%;
  max-width: 320px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  align-items: stretch;
}

.button-row {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.control-btn {
  width: 100%;
  min-height: 42px;
  padding: 0 14px;
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  font: inherit;
  font-size: 0.92rem;
  font-weight: 650;
  cursor: pointer;
  transition:
    background-color var(--motion-fast) var(--motion-ease),
    border-color var(--motion-fast) var(--motion-ease),
    color var(--motion-fast) var(--motion-ease),
    filter var(--motion-fast) var(--motion-ease);

  .el-icon {
    font-size: 16px;
  }

  &:disabled {
    opacity: 0.56;
    cursor: not-allowed;
  }

  &:not(:disabled):hover {
    filter: brightness(0.98);
  }

  &.compact {
    min-height: 38px;
    font-size: 0.86rem;
  }

  &.primary {
    background: var(--brand-primary);
    color: var(--text-inverse);
  }

  &.warning {
    background: color-mix(in srgb, var(--status-warning) 16%, var(--bg-elevated));
    color: var(--status-warning);
    border-color: color-mix(in srgb, var(--status-warning) 22%, var(--border-subtle));
  }

  &.danger {
    background: color-mix(in srgb, var(--status-error) 14%, var(--bg-elevated));
    color: var(--status-error);
    border-color: color-mix(in srgb, var(--status-error) 20%, var(--border-subtle));
  }

  &.secondary {
    background: var(--bg-elevated);
    color: var(--text-primary);
    border-color: var(--border-subtle);
  }

  &.ghost-danger {
    background: transparent;
    color: var(--status-error);
    border-color: color-mix(in srgb, var(--status-error) 18%, var(--border-subtle));
  }
}

.return-link {
  align-self: center;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font: inherit;
  font-weight: 600;
  font-size: 0.88rem;
  cursor: pointer;
  padding: 6px 12px;
  transition: color var(--motion-fast) var(--motion-ease);

  &:hover {
    color: var(--text-primary);
  }
}

@media (max-width: 768px) {
  .button-stack {
    max-width: 100%;
  }
}
</style>
