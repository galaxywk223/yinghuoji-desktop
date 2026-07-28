<!-- 专注信息显示组件 -->
<template>
  <div class="focus-info">
    <div class="info-item">
      <span class="info-label">计时模式</span>
      <span class="info-value">
        {{ formData.mode === "countdown" ? "倒计时" : "正计时" }}
        <template v-if="formData.mode === 'countdown'">
          · {{ formData.durationMinutes }} 分钟
        </template>
      </span>
    </div>
    <div class="info-item">
      <span class="info-label">记录名称</span>
      <span class="info-value">{{ formData.name }}</span>
    </div>
    <div class="info-item">
      <span class="info-label">分类</span>
      <span class="info-value">
        <span
          v-if="currentCategory?.color"
          class="category-dot"
          :style="{ background: currentCategory.color }"
        />
        {{ currentCategory?.name || "—" }}
      </span>
    </div>
    <div v-if="currentSubcategory" class="info-item">
      <span class="info-label">子分类</span>
      <span class="info-value">{{ currentSubcategory.name }}</span>
    </div>
    <div v-if="formData.notes" class="info-item">
      <span class="info-label">备注</span>
      <span class="info-value">{{ formData.notes }}</span>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";

const props = defineProps({
  formData: {
    type: Object,
    required: true,
  },
  categories: {
    type: Array,
    default: () => [],
  },
  subcategories: {
    type: Array,
    default: () => [],
  },
});

const currentCategory = computed(() => {
  return props.categories.find((cat) => cat.id === props.formData.categoryId);
});

const currentSubcategory = computed(() => {
  return props.subcategories.find(
    (sub) => sub.id === props.formData.subcategoryId,
  );
});
</script>

<style scoped lang="scss">
.focus-info {
  width: 100%;
  margin: 0;
  padding: 4px 2px;
  display: flex;
  flex-direction: column;
  gap: 0;
  border: 0;
  background: transparent;
  box-shadow: none;
}

.info-item {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 16px;
  padding: 12px 2px;

  &:not(:last-child) {
    border-bottom: 1px solid var(--border-subtle);
  }
}

.info-label {
  flex-shrink: 0;
  font-weight: 650;
  color: var(--text-muted);
  font-size: 0.82rem;
}

.info-value {
  flex: 1;
  min-width: 0;
  text-align: right;
  color: var(--text-primary);
  font-size: 0.92rem;
  font-weight: 600;
  line-height: 1.4;
  word-break: break-word;
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}

.category-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
</style>
