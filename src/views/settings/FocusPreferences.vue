<template>
  <PageContainer
    :title="{ icon: 'lucide:timer-reset', text: '专注偏好' }"
    subtitle="配置专注计时自动生成记录时使用的时间规则"
    :custom-class="'settings-subpage'"
    max-width="full"
    fill-height
  >
    <div class="focus-preferences-workbench">
      <section class="preference-card">
        <div class="preference-card__header">
          <div>
            <p class="preference-eyebrow">学习日</p>
            <h4>凌晨分界点</h4>
          </div>
          <span class="boundary-badge">{{ boundaryLabel }}</span>
        </div>

        <p class="preference-description">
          专注计时保存记录时，早于该时间点结束的记录归入前一天。
        </p>

        <div class="boundary-control">
          <el-slider
            v-model="boundaryHour"
            :min="MIN_FOCUS_DAY_BOUNDARY_HOUR"
            :max="MAX_FOCUS_DAY_BOUNDARY_HOUR"
            :step="1"
            show-stops
            @change="saveBoundaryHour"
          />
          <el-input-number
            v-model="boundaryHour"
            :min="MIN_FOCUS_DAY_BOUNDARY_HOUR"
            :max="MAX_FOCUS_DAY_BOUNDARY_HOUR"
            :step="1"
            controls-position="right"
            @change="saveBoundaryHour"
          />
        </div>

        <div class="example-panel">
          <span class="example-label">规则示例</span>
          <p>
            记录在 {{ exampleEarlyTime }} 结束时归入前一天；记录在
            {{ exampleBoundaryTime }} 及以后结束时归入当天。
          </p>
        </div>
      </section>

      <aside class="preference-support-panel">
        <section>
          <p class="support-eyebrow">默认值</p>
          <h4>{{ defaultBoundaryLabel }}</h4>
          <p>默认覆盖午夜后到凌晨四点前结束的专注记录。</p>
        </section>
        <section>
          <p class="support-eyebrow">影响范围</p>
          <h4>仅影响专注计时</h4>
          <p>手动新增记录和历史数据保持原有日期，不执行自动迁移。</p>
        </section>
      </aside>
    </div>
  </PageContainer>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import PageContainer from "@/components/layout/PageContainer.vue";
import { useSettingsStore } from "@/stores/modules/settings";
import {
  DEFAULT_FOCUS_DAY_BOUNDARY_HOUR,
  MAX_FOCUS_DAY_BOUNDARY_HOUR,
  MIN_FOCUS_DAY_BOUNDARY_HOUR,
  normalizeFocusDayBoundaryHour,
} from "@/utils/focusLearningDay";

defineOptions({ name: "FocusPreferencesSettingsView" });

const settingsStore = useSettingsStore();
const boundaryHour = ref(settingsStore.focusDayBoundaryHour);

watch(
  () => settingsStore.focusDayBoundaryHour,
  (value) => {
    boundaryHour.value = value;
  },
);

const formatBoundaryLabel = (hour: number) =>
  `${String(hour).padStart(2, "0")}:00`;

const boundaryLabel = computed(() =>
  formatBoundaryLabel(normalizeFocusDayBoundaryHour(boundaryHour.value)),
);
const defaultBoundaryLabel = formatBoundaryLabel(
  DEFAULT_FOCUS_DAY_BOUNDARY_HOUR,
);
const exampleBoundaryTime = computed(() => boundaryLabel.value);
const exampleEarlyTime = computed(() => {
  const hour = normalizeFocusDayBoundaryHour(boundaryHour.value);
  if (hour <= 0) return "00:00 前";
  return `${String(hour - 1).padStart(2, "0")}:59`;
});

function saveBoundaryHour(value: number | undefined) {
  const nextHour = normalizeFocusDayBoundaryHour(
    value ?? boundaryHour.value ?? DEFAULT_FOCUS_DAY_BOUNDARY_HOUR,
  );
  boundaryHour.value = nextHour;
  settingsStore.setFocusDayBoundaryHour(nextHour);
  ElMessage.success(`凌晨分界点已设为 ${formatBoundaryLabel(nextHour)}`);
}
</script>

<style scoped>
.focus-preferences-workbench {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 300px;
  gap: 18px;
  align-items: start;
}

.preference-card,
.preference-support-panel {
  border: 1px solid var(--stroke-soft);
  border-radius: 14px;
  background: var(--surface-card);
  box-shadow: var(--box-shadow-card);
}

.preference-card {
  display: flex;
  flex-direction: column;
  gap: 18px;
  padding: 20px;
}

.preference-card__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.preference-eyebrow,
.support-eyebrow {
  margin: 0 0 6px;
  color: var(--color-text-muted);
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.preference-card h4,
.preference-support-panel h4 {
  margin: 0;
  color: var(--color-text-heading);
  font-size: 17px;
  font-weight: 800;
}

.boundary-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 72px;
  min-height: 34px;
  padding: 0 12px;
  border-radius: 8px;
  background: var(--color-primary-light);
  color: var(--color-primary);
  font-weight: 800;
}

.preference-description {
  margin: 0;
  color: var(--color-text-secondary);
  font-size: 14px;
  line-height: 1.7;
}

.boundary-control {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 132px;
  gap: 18px;
  align-items: center;
  padding: 16px;
  border-radius: 12px;
  background: var(--surface-card-muted);
  border: 1px solid var(--stroke-soft);
}

.example-panel {
  padding: 14px 16px;
  border-radius: 12px;
  background: var(--surface-card-muted);
  border: 1px solid var(--stroke-soft);
}

.example-label {
  display: block;
  margin-bottom: 8px;
  color: var(--color-text-muted);
  font-size: 12px;
  font-weight: 800;
}

.example-panel p,
.preference-support-panel p {
  margin: 0;
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1.65;
}

.preference-support-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 14px;
}

.preference-support-panel section {
  padding: 14px 16px;
  border-radius: 12px;
  background: var(--surface-card-muted);
  border: 1px solid var(--stroke-soft);
}

.preference-support-panel h4 {
  margin-bottom: 10px;
  font-size: 15px;
}

@media (max-width: 900px) {
  .focus-preferences-workbench {
    grid-template-columns: 1fr;
  }

  .boundary-control {
    grid-template-columns: 1fr;
  }
}
</style>
