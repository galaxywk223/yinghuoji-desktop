<!-- 专注计时器显示组件 -->
<template>
  <div
    class="timer-display"
    :class="{
      'timer-active': isActive,
      'timer-countdown': timerMode === 'countdown',
      'timer-completed': status === 'completed' && timerMode === 'countdown',
    }"
  >
    <div class="time-circle">
      <svg class="progress-ring" :width="ringSize" :height="ringSize">
        <circle
          class="progress-ring-bg outer"
          :cx="center"
          :cy="center"
          :r="outerRadius"
          fill="none"
          stroke-width="8"
        />
        <circle
          class="progress-ring-circle outer"
          :cx="center"
          :cy="center"
          :r="outerRadius"
          fill="none"
          stroke-width="8"
          :stroke-dasharray="outerCircumference"
          :stroke-dashoffset="outerProgressOffset"
        />
        <circle
          class="progress-ring-bg inner"
          :cx="center"
          :cy="center"
          :r="innerRadius"
          fill="none"
          stroke-width="5"
        />
        <circle
          class="progress-ring-circle inner"
          :cx="center"
          :cy="center"
          :r="innerRadius"
          fill="none"
          stroke-width="5"
          :stroke-dasharray="innerCircumference"
          :stroke-dashoffset="innerProgressOffset"
        />
      </svg>
      <div class="time-text" aria-live="polite">
        <span
          v-if="status === 'completed' && timerMode === 'countdown'"
          class="complete-mark"
        >
          到时
        </span>
        <span class="time-value">{{ formattedTime }}</span>
        <span class="time-label">{{ timeLabel }}</span>
        <span class="time-hint">{{ progressHint }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";

const props = defineProps({
  elapsedSeconds: {
    type: Number,
    default: 0,
  },
  isActive: {
    type: Boolean,
    default: false,
  },
  displaySeconds: {
    type: Number,
    default: 0,
  },
  targetDurationSeconds: {
    type: Number,
    default: 0,
  },
  countdownProgress: {
    type: Number,
    default: 0,
  },
  timerMode: {
    type: String,
    default: "countup",
  },
  status: {
    type: String,
    default: "idle",
  },
});

const ringSize = 280;
const center = ringSize / 2;
const outerRadius = center - 12;
const innerRadius = center - 28;
const outerCircumference = 2 * Math.PI * outerRadius;
const innerCircumference = 2 * Math.PI * innerRadius;

const innerCycleSeconds = 60 * 60;
const outerCycleSeconds = 12 * 60 * 60;

function calcCycleOffset(elapsed, cycle, circumference) {
  if (elapsed <= 0) {
    return circumference;
  }
  const remainder = elapsed % cycle;
  const progress = remainder === 0 ? 1 : remainder / cycle;
  return circumference - progress * circumference;
}

const innerProgressOffset = computed(() => {
  if (props.timerMode === "countdown") {
    if (props.displaySeconds <= 0) return innerCircumference;
    const secondsInMinute = props.displaySeconds % 60;
    const minuteProgress = secondsInMinute === 0 ? 1 : secondsInMinute / 60;
    return innerCircumference - minuteProgress * innerCircumference;
  }
  return calcCycleOffset(
    props.elapsedSeconds,
    innerCycleSeconds,
    innerCircumference,
  );
});

const outerProgressOffset = computed(() => {
  if (props.timerMode === "countdown") {
    const progress = Math.min(1, Math.max(0, props.countdownProgress));
    return outerCircumference - progress * outerCircumference;
  }
  return calcCycleOffset(
    props.elapsedSeconds,
    outerCycleSeconds,
    outerCircumference,
  );
});

const formattedTime = computed(() => {
  const value =
    props.timerMode === "countdown"
      ? props.displaySeconds
      : props.elapsedSeconds;
  const hours = Math.floor(value / 3600);
  const minutes = Math.floor((value % 3600) / 60);
  const seconds = value % 60;
  return [hours, minutes, seconds]
    .map((unit) => unit.toString().padStart(2, "0"))
    .join(":");
});

const timeLabel = computed(() => {
  return props.timerMode === "countdown" ? "剩余时间" : "已专注";
});

const progressHint = computed(() => {
  if (props.timerMode === "countdown") {
    if (props.status === "completed") return "本轮专注已完成";
    const totalMinutes = Math.round(props.targetDurationSeconds / 60);
    if (totalMinutes >= 60 && totalMinutes % 60 === 0) {
      return `共 ${totalMinutes / 60} 小时`;
    }
    return `共 ${totalMinutes} 分钟`;
  }
  return "内环 1h · 外环 12h";
});
</script>

<style scoped lang="scss">
.timer-display {
  display: flex;
  justify-content: center;
  align-items: center;
  margin: 0;
  position: relative;
  min-height: 280px;

  .time-circle {
    position: relative;
    display: flex;
    justify-content: center;
    align-items: center;

    .progress-ring {
      transform: rotate(-90deg);

      &-bg {
        transition: stroke 0.3s ease;

        &.outer {
          stroke: var(--bg-muted);
        }

        &.inner {
          stroke: color-mix(in srgb, var(--border-subtle) 80%, var(--bg-elevated));
        }
      }

      &-circle {
        stroke-linecap: round;
        transition: stroke 0.3s ease;

        &.outer {
          stroke: var(--brand-primary);
          transition:
            stroke-dashoffset 0.25s ease,
            stroke 0.3s ease;
        }

        &.inner {
          stroke: var(--brand-accent);
          transition:
            stroke-dashoffset 0.2s linear,
            stroke 0.3s ease;
        }
      }
    }

    .time-text {
      position: absolute;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      width: 68%;
      padding: 0 6px;

      .time-value {
        font-size: clamp(1.7rem, 5.2vw, 2.3rem);
        font-weight: 700;
        color: var(--text-primary);
        letter-spacing: 0.02em;
        font-family: "SFMono-Regular", "JetBrains Mono", "Consolas", monospace;
        font-variant-numeric: tabular-nums;
        line-height: 1;
        max-width: 100%;
        text-align: center;
        white-space: nowrap;
        margin-bottom: 0.3rem;
      }

      .time-label {
        font-size: 0.78rem;
        color: var(--text-muted);
        margin-top: 0.35rem;
        letter-spacing: 0.06em;
        text-transform: uppercase;
        font-weight: 700;
      }

      .time-hint {
        margin-top: 0.35rem;
        font-size: 0.74rem;
        color: var(--text-secondary);
        letter-spacing: 0;
      }

      .complete-mark {
        margin-bottom: 8px;
        color: var(--status-success);
        font-size: 0.82rem;
        font-weight: 800;
      }
    }
  }

  &.timer-active {
    .progress-ring-circle.outer {
      stroke: var(--brand-accent);
    }

    .progress-ring-circle.inner {
      stroke: var(--brand-primary-strong);
    }
  }

  &.timer-countdown .progress-ring-circle {
    &.outer {
      stroke: var(--brand-primary);
    }

    &.inner {
      stroke: var(--brand-accent);
    }
  }

  &.timer-completed {
    .progress-ring-circle.outer,
    .progress-ring-circle.inner {
      stroke: var(--status-success);
    }

    .time-circle {
      animation: focus-complete-pulse 1.4s ease-in-out infinite;
    }
  }
}

@keyframes focus-complete-pulse {
  0%,
  100% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.02);
  }
}

@media (prefers-reduced-motion: reduce) {
  .timer-display.timer-completed .time-circle {
    animation: none;
  }
}
</style>
