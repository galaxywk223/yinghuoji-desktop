<!-- 周折叠面板组件 -->
<template>
  <el-collapse
    :collapse-transition="false"
    :model-value="activeWeeks"
    class="weeks-accordion"
    @update:model-value="$emit('update:activeWeeks', $event)"
  >
    <el-collapse-item
      v-for="week in weeks"
      :key="`${week.year}-${week.week_num}`"
      :name="`${week.year}-${week.week_num}`"
    >
      <!-- 周标题 -->
      <template #title>
        <div class="week-header">
          <span class="week-title" :title="formatWeekRange(week)">
            <span class="emoji-icon" aria-hidden="true">📅</span>
            <span class="week-title-text"
              >{{ formatWeekTitle(week) }}</span
            >
          </span>
          <div
            class="week-progress-panel"
            :title="`本周总时长: ${getWeekTotalDuration(week)} 分钟`"
          >
            <span class="week-progress-label">本周投入</span>
            <el-progress
              :percentage="getWeekProgressPercentage(week)"
              :show-text="false"
              :stroke-width="8"
              :color="getWeekProgressColor(week)"
            />
            <strong class="week-progress-hours">{{ formatWeekDuration(week) }}</strong>
            <span class="week-progress-count">{{ getWeekRecordCount(week) }} 条记录</span>
          </div>
          <span class="week-eff">
            <Icon icon="lucide:gauge" class="week-eff-icon" />
            平均效率 {{ formatWeekEfficiency(week) }}
          </span>
        </div>
      </template>

      <!-- 每周的每一天 -->
      <div class="week-days">
        <DayCard
          v-for="day in week.days"
          :key="day.date"
          :day="day"
          :expanded-notes="expandedNotes"
          :color-seed="colorSeed"
          @add-record="$emit('add-record', $event)"
          @toggle-notes="$emit('toggle-notes', $event)"
          @edit-record="$emit('edit-record', $event)"
          @delete-record="$emit('delete-record', $event)"
        />
      </div>
    </el-collapse-item>
  </el-collapse>
</template>

<script setup>
import { Icon } from "@iconify/vue";
import DayCard from "./DayCard.vue";

// Props
const props = defineProps({
  weeks: {
    type: Array,
    default: () => [],
  },
  activeWeeks: {
    type: Array,
    default: () => [],
  },
  expandedNotes: {
    type: Array,
    default: () => [],
  },
  colorSeed: {
    type: String,
    default: "",
  },
  stageName: {
    type: String,
    default: "",
  },
});

// Emits
defineEmits([
  "add-record",
  "toggle-notes",
  "edit-record",
  "delete-record",
  "update:activeWeeks",
]);

const WEEK_TARGET_MINUTES = 840 * 7;

const getWeekRecordCount = (week) =>
  (week.days || []).reduce((count, day) => count + (day.logs?.length || 0), 0);

const getWeekTotalDuration = (week) =>
  (week.days || []).reduce(
    (sum, day) => sum + (day.total_duration || 0),
    0,
  );

const formatWeekDuration = (week) => {
  const totalMinutes = getWeekTotalDuration(week);
  if (!totalMinutes) return "0h";
  return `${(totalMinutes / 60).toFixed(totalMinutes >= 600 ? 0 : 1)}h`;
};

const formatWeekRange = (week) =>
  week.week_start && week.week_end ? `${week.week_start} ~ ${week.week_end}` : "";

const formatWeekTitle = (week) => {
  const prefix = props.stageName?.trim();
  return prefix ? `${prefix} · 第 ${week.week_num} 周` : `第 ${week.week_num} 周`;
};

const getWeekProgressPercentage = (week) =>
  Math.min(100, (getWeekTotalDuration(week) / WEEK_TARGET_MINUTES) * 100);

const getWeekProgressColor = (week) => {
  const percentage = getWeekProgressPercentage(week);
  if (percentage >= 80) return "#10b981";
  if (percentage >= 50) return "#667eea";
  if (percentage >= 30) return "#fbbf24";
  return "#ef4444";
};

const formatWeekEfficiency = (week) => {
  const value = Number(week.efficiency);
  return Number.isFinite(value) ? value.toFixed(2) : "--";
};
</script>

<style scoped lang="scss">
.weeks-accordion {
  :deep(.el-collapse) {
    border: none;
  }

  :deep(.el-collapse-item) {
    margin-bottom: 1.25rem;
    padding: 24px;
    border-radius: 28px;
    border: 1px solid color-mix(in srgb, var(--color-primary) 10%, var(--stroke-soft));
    background:
      radial-gradient(circle at top right, color-mix(in srgb, var(--color-primary) 10%, transparent) 0%, transparent 28%),
      linear-gradient(
        180deg,
        color-mix(in srgb, var(--surface-card) 92%, rgba(255, 255, 255, 0.03)) 0%,
        color-mix(in srgb, var(--surface-card-strong) 96%, rgba(15, 23, 42, 0.12)) 100%
      );
    box-shadow:
      0 22px 44px -34px rgba(15, 23, 42, 0.5),
      inset 0 1px 0 rgba(255, 255, 255, 0.05);
  }

  :deep(.el-collapse-item__header) {
    height: auto;
    line-height: normal;
    padding: 0 0 1rem 0;
    background: transparent;
    border-bottom: none;
    margin-bottom: 0.5rem;

    .el-collapse-item__arrow {
      display: none;
    }
  }

  :deep(.el-collapse-item__wrap) {
    border-bottom: none;
    background: transparent;
  }

  :deep(.el-collapse-item__content) {
    padding: 0;
    background: transparent;
  }

  .week-header {
    display: grid;
    grid-template-columns: minmax(260px, auto) minmax(300px, 1fr) auto;
    align-items: center;
    gap: 16px;
    width: 100%;
    min-width: 0;
  }

  .week-title {
    font-size: 24px;
    font-weight: 800;
    color: var(--color-text-heading);
    letter-spacing: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    white-space: nowrap;

    .emoji-icon {
      font-size: 22px;
      flex-shrink: 0;
    }
  }

  .week-title-text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .week-progress-panel {
    min-width: 0;
    display: grid;
    grid-template-columns: max-content minmax(120px, 1fr) max-content max-content;
    align-items: center;
    gap: 12px;
    padding: 0;
    border: 0;
    border-radius: 0;
    background: transparent;
    box-shadow: none;

    .week-progress-label {
      font-size: 12px;
      font-weight: 600;
      letter-spacing: 0.04em;
      color: var(--color-text-secondary);
      white-space: nowrap;
    }

    .week-progress-hours {
      font-size: 16px;
      color: var(--color-text-heading);
      white-space: nowrap;
    }

    .week-progress-count {
      font-size: 13px;
      font-weight: 600;
      color: var(--color-text-secondary);
      white-space: nowrap;
    }

    :deep(.el-progress) {
      min-width: 0;
      width: 100%;
      line-height: 0;
    }

    :deep(.el-progress-bar__outer) {
      background-color: var(--color-bg-hover);
    }
  }

  .week-eff {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    justify-self: end;
    padding: 10px 16px;
    border-radius: 999px;
    background: linear-gradient(
      135deg,
      color-mix(in srgb, var(--color-primary) 14%, rgba(255, 255, 255, 0.03)) 0%,
      color-mix(in srgb, var(--color-primary-dark) 10%, rgba(255, 255, 255, 0.02)) 100%
    );
    border: 1px solid color-mix(in srgb, var(--color-primary) 16%, transparent);
    font-size: 15px;
    color: var(--color-text-heading);
    font-weight: 700;
    white-space: nowrap;
  }

  .week-eff-icon {
    width: 14px;
    height: 14px;
    color: var(--color-primary);
  }

  .week-days {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
}

@media (max-width: 768px) {
  .weeks-accordion {
    :deep(.el-collapse-item) {
      padding: 18px;
      border-radius: 24px;
    }

    .week-header {
      grid-template-columns: 1fr;
      align-items: stretch;
    }

    .week-title {
      font-size: 21px;
    }

    .week-eff {
      justify-self: start;
      align-self: flex-start;
    }
  }
}
</style>
