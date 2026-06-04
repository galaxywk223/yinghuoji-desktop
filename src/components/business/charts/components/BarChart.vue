<template>
  <div class="bar-card">
    <header class="bar-card__header">
      <div class="bar-card__title">
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M3 13h5v8H3v-8Zm6-6h5v14h-5V7Zm6 4h6v10h-6V11Z" />
        </svg>
        <h5>{{ title }}</h5>
      </div>
    </header>
    <div ref="scrollWrapper" class="bar-card__list">
      <div
        v-for="item in displayItems"
        :key="item.name"
        class="bar-item"
        @click="emitClick(item.name)"
        @mouseenter="emitHover(item.name)"
        @mouseleave="emitLeave"
      >
        <div class="bar-info">
          <span class="bar-label">
            <span class="bar-rank">{{ item.rank }}</span>
            <span class="bar-name">{{ item.name }}</span>
          </span>
          <span class="bar-value"
            >{{ formatValue(item.value) }}{{ unitSuffix }}</span
          >
        </div>
        <div class="bar-track">
          <div
            class="bar-fill"
            :style="{
              width: item.percent + '%',
              backgroundColor: item.color,
            }"
          ></div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, ref } from "vue";

const props = defineProps({
  data: { type: Object, required: true },
  title: { type: String, default: "High Frequency Categories" },
  colors: { type: Array, default: () => [] },
  metricMode: { type: String, default: "duration" }, // 'duration' | 'efficiency'
  valueUnit: { type: String, default: "" },
});

const emit = defineEmits(["bar-click", "bar-hover", "bar-leave"]);
const scrollWrapper = ref(null);

const unitSuffix = computed(() =>
  props.valueUnit || (props.metricMode === "efficiency" ? "" : "h"),
);

function scrollToTop(smooth = true) {
  const el = scrollWrapper.value;
  if (!el) return;
  if (typeof el.scrollTo === "function") {
    el.scrollTo({ top: 0, behavior: smooth ? "smooth" : "auto" });
  } else {
    el.scrollTop = 0;
  }
}
defineExpose({ scrollToTop });

const normalized = computed(() => {
  const labels = Array.isArray(props.data?.labels) ? props.data.labels : [];
  const values = Array.isArray(props.data?.data) ? props.data.data : [];

  return labels.map((label, idx) => {
    const hasLabel = typeof label === "string" && label.trim().length > 0;
    const name = hasLabel ? label.trim() : `分类 ${idx + 1}`;
    const rawValue = values[idx];
    const numeric = Number(rawValue ?? 0);
    return { name, value: Number.isFinite(numeric) ? numeric : 0 };
  });
});

const sortedData = computed(() => {
  const items = [...normalized.value];
  return items.sort((a, b) => {
    if (b.value === a.value) {
      return a.name.localeCompare(b.name);
    }
    return b.value - a.value;
  });
});

const barColors = computed(() => {
  if (props.colors?.length) {
    return props.colors;
  }
  return [
    "#6366f1",
    "#22d3ee",
    "#f97316",
    "#0ea5e9",
    "#facc15",
    "#10b981",
    "#f472b6",
    "#fb7185",
    "#14b8a6",
    "#8b5cf6",
  ];
});

const maxValue = computed(() =>
  Math.max(0, ...sortedData.value.map((item) => Math.abs(item.value))),
);

const displayItems = computed(() => {
  const max = maxValue.value || 1;
  return sortedData.value.map((item, idx) => {
    const magnitude = Math.abs(Number(item.value || 0));
    const percent = Math.max(
      0,
      Math.min(100, (magnitude / max) * 100),
    );
    return {
      ...item,
      rank: formatRank(idx + 1),
      color: barColors.value[idx % barColors.value.length],
      percent: Number(percent.toFixed(2)),
    };
  });
});

const formatValue = (val) => Number(val ?? 0).toFixed(1);
const formatRank = (rank) => (rank < 10 ? `0${rank}` : String(rank));

const emitClick = (name) => {
  if (typeof name === "string" && name.trim()) emit("bar-click", name);
};
const emitHover = (name) => {
  if (typeof name === "string" && name.trim()) emit("bar-hover", name);
};
const emitLeave = () => emit("bar-leave");
</script>

<style scoped lang="scss">
.bar-card {
  background: var(--surface-card);
  border-radius: 24px;
  border: 1px solid var(--color-border-card);
  box-shadow: var(--box-shadow-card);
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-height: 280px;
  transition:
    transform 0.3s ease,
    box-shadow 0.3s ease;

  &:hover {
    transform: translateY(-2px);
    box-shadow: var(--box-shadow-hover);
  }
}

.bar-card__header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.bar-card__title {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--color-text-heading);

  svg {
    width: 20px;
    height: 20px;
    color: var(--color-primary);
  }

  h5 {
    margin: 0;
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.5px;
  }
}

.bar-card__list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-height: 420px;
  overflow-y: auto;
  margin-right: -8px;
  padding-right: 8px;
}

.bar-card__list::-webkit-scrollbar {
  width: 4px;
}

.bar-card__list::-webkit-scrollbar-thumb {
  background: var(--surface-soft);
  border-radius: 999px;
}

.bar-item {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 12px;
  border-radius: 12px;
  transition: background-color 0.2s ease;
  cursor: pointer;
}

.bar-item:hover {
  background: var(--surface-subtle);
}

.bar-info {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.bar-label {
  display: inline-flex;
  align-items: center;
  flex: 1 1 auto;
  gap: 10px;
  min-width: 0;
}

.bar-rank {
  width: 28px;
  flex: 0 0 28px;
  font-size: 12px;
  font-weight: 700;
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.02em;
}

.bar-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--color-text-base);
  font-size: 14px;
  font-weight: 600;
}

.bar-value {
  font-weight: 600;
  color: var(--color-text-secondary);
  font-size: 13px;
  flex-shrink: 0;
}

.bar-track {
  width: 100%;
  height: 8px;
  background: var(--surface-subtle);
  border-radius: 999px;
  overflow: hidden;
}

.bar-fill {
  height: 100%;
  border-radius: 999px;
  transition: width 0.6s cubic-bezier(0.4, 0, 0.2, 1);
}
</style>
