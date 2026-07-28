<template>
  <div class="focus-form">
    <el-form
      ref="formRef"
      :model="localForm"
      :rules="rules"
      label-position="top"
    >
      <el-form-item label="计时模式" prop="mode">
        <div class="timer-mode-control" role="group" aria-label="计时模式">
          <button
            type="button"
            :class="{ active: localForm.mode === 'countup' }"
            @click="localForm.mode = 'countup'"
          >
            正计时
          </button>
          <button
            type="button"
            :class="{ active: localForm.mode === 'countdown' }"
            @click="localForm.mode = 'countdown'"
          >
            倒计时
          </button>
        </div>
      </el-form-item>

      <el-form-item
        v-if="localForm.mode === 'countdown'"
        label="专注时长"
        prop="durationMinutes"
      >
        <div class="duration-control">
          <div class="duration-presets" aria-label="常用专注时长">
            <button
              v-for="minutes in durationPresets"
              :key="minutes"
              type="button"
              :class="{ active: localForm.durationMinutes === minutes }"
              @click="localForm.durationMinutes = minutes"
            >
              {{ formatPresetLabel(minutes) }}
            </button>
          </div>
          <div class="custom-duration">
            <span class="custom-duration__label">自定义</span>
            <el-input-number
              v-model="localForm.durationMinutes"
              :min="1"
              :max="720"
              :step="5"
              controls-position="right"
              aria-label="自定义倒计时分钟数"
            />
            <span class="custom-duration__unit">分钟</span>
          </div>
        </div>
      </el-form-item>

      <el-form-item label="记录名称" prop="name">
        <template #label>
          <span class="required-star">*</span> 记录名称
        </template>
        <el-input
          v-model="localForm.name"
          placeholder="请输入本次专注的内容"
          :maxlength="50"
          show-word-limit
          size="large"
        />
      </el-form-item>

      <div class="category-row">
        <el-form-item label="分类" prop="categoryId" class="category-item">
          <template #label>
            <span class="required-star">*</span> 分类
          </template>
          <el-select
            v-model="localForm.categoryId"
            placeholder="请选择分类"
            style="width: 100%"
            size="large"
            filterable
            @change="onCategoryChange"
            popper-class="dark-dropdown"
          >
            <el-option
              v-for="cat in categories"
              :key="cat.id"
              :label="cat.name"
              :value="cat.id"
            >
              <span :style="{ color: cat.color }">● </span>
              <span>{{ cat.name }}</span>
            </el-option>
          </el-select>
        </el-form-item>

        <el-form-item label="子分类" prop="subcategoryId" class="category-item">
          <el-select
            v-model="localForm.subcategoryId"
            placeholder="请选择子分类"
            class="subcategory-select"
            style="width: 100%"
            size="large"
            filterable
            :disabled="!localForm.categoryId || !availableSubcategories.length"
          >
            <el-option
              v-for="subcat in availableSubcategories"
              :key="subcat.id"
              :label="subcat.name"
              :value="subcat.id"
            />
          </el-select>
        </el-form-item>
      </div>
    </el-form>
  </div>
</template>

<script setup>
import { computed, watch, ref, nextTick, reactive } from "vue";

const formRef = ref(null);

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

const emit = defineEmits(["update:formData", "category-change"]);

const localForm = reactive({
  name: "",
  categoryId: null,
  subcategoryId: null,
  mode: "countup",
  durationMinutes: 30,
});

const durationPresets = [30, 45, 60, 90, 120, 150, 180, 240];
const syncing = ref(false);

const formatPresetLabel = (minutes) => {
  if (minutes >= 60 && minutes % 60 === 0) {
    return `${minutes / 60} 小时`;
  }
  return `${minutes} 分`;
};

watch(
  () => props.formData,
  (value) => {
    syncing.value = true;
    Object.assign(localForm, value || {});
    nextTick(() => {
      syncing.value = false;
    });
  },
  { immediate: true },
);

watch(
  () => [
    localForm.name,
    localForm.categoryId,
    localForm.subcategoryId,
    localForm.mode,
    localForm.durationMinutes,
  ],
  () => {
    if (syncing.value) return;
    emit("update:formData", { ...localForm });
  },
  { deep: true },
);

const rules = {
  name: [
    { required: true, message: "请输入记录名称", trigger: "blur" },
    { min: 1, max: 50, message: "长度在 1 到 50 个字符", trigger: "blur" },
  ],
  categoryId: [{ required: true, message: "请选择分类", trigger: "change" }],
  durationMinutes: [
    {
      validator: (_rule, value, callback) => {
        if (localForm.mode !== "countdown") return callback();
        if (!Number.isInteger(value) || value < 1 || value > 720) {
          return callback(new Error("倒计时时长需为 1 到 720 分钟"));
        }
        callback();
      },
      trigger: "change",
    },
  ],
};

const availableSubcategories = computed(() => {
  if (!localForm.categoryId) return [];
  return props.subcategories.filter(
    (sub) => sub.category_id === localForm.categoryId,
  );
});

const onCategoryChange = () => {
  localForm.subcategoryId = null;
  emit("category-change", localForm.categoryId);
};

defineExpose({
  validate: () => formRef.value?.validate(),
});
</script>

<style scoped lang="scss">
.focus-form {
  width: 100%;
  margin: 0;

  :deep(.el-form) {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 0;
    background: transparent;
    border: 0;
    box-shadow: none;
  }

  :deep(.el-form-item) {
    margin-bottom: 0;
  }

  :deep(.el-form-item__label) {
    color: var(--text-primary);
    font-weight: 700;
    font-size: 0.88rem;
    padding-bottom: 6px;
    line-height: 1.2;
  }

  .timer-mode-control {
    width: 100%;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    padding: 4px;
    border-radius: var(--radius-md);
    background: var(--bg-muted);
    border: 1px solid var(--border-subtle);

    button {
      min-height: 36px;
      border: 0;
      border-radius: calc(var(--radius-md) - 2px);
      background: transparent;
      color: var(--text-secondary);
      font: inherit;
      font-size: 0.9rem;
      font-weight: 700;
      cursor: pointer;
      transition:
        background-color var(--motion-fast) var(--motion-ease),
        color var(--motion-fast) var(--motion-ease);

      &.active {
        background: var(--bg-surface);
        color: var(--brand-primary);
        box-shadow: var(--shadow-1);
      }
    }
  }

  .duration-control {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .duration-presets {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;

    button {
      min-height: 36px;
      padding: 0 6px;
      border: 1px solid var(--border-subtle);
      border-radius: var(--radius-md);
      background: var(--bg-surface);
      color: var(--text-secondary);
      font: inherit;
      font-size: 0.84rem;
      font-weight: 650;
      cursor: pointer;
      transition:
        border-color var(--motion-fast) var(--motion-ease),
        background-color var(--motion-fast) var(--motion-ease),
        color var(--motion-fast) var(--motion-ease);

      &.active {
        border-color: color-mix(in srgb, var(--brand-primary) 45%, var(--border-subtle));
        background: color-mix(in srgb, var(--brand-primary) 12%, var(--bg-surface));
        color: var(--brand-primary-strong);
      }

      &:hover:not(.active) {
        border-color: var(--border-strong);
        color: var(--text-primary);
      }
    }
  }

  .custom-duration {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 40px;
    padding: 6px 10px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--bg-muted);
    color: var(--text-secondary);
    font-size: 0.88rem;

    &__label {
      flex-shrink: 0;
      font-weight: 650;
      color: var(--text-muted);
    }

    &__unit {
      flex-shrink: 0;
      color: var(--text-secondary);
    }

    :deep(.el-input-number) {
      width: 128px;
    }
  }

  .category-row {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;

    @media (max-width: 768px) {
      grid-template-columns: 1fr;
    }
  }

  .category-item {
    margin-bottom: 0;
  }

  .required-star {
    color: var(--status-error);
    margin-right: 2px;
    font-weight: 700;
  }

  :deep(.el-input__wrapper),
  :deep(.el-select .el-input__wrapper) {
    background: var(--bg-muted) !important;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    box-shadow: none !important;
    padding: 4px 12px;
    min-height: 40px;
    transition:
      border-color var(--motion-fast) var(--motion-ease),
      box-shadow var(--motion-fast) var(--motion-ease);

    &:hover {
      border-color: var(--border-strong);
    }

    &.is-focus {
      border-color: var(--brand-primary);
      box-shadow: var(--focus-ring) !important;
    }
  }

  :deep(.el-input__inner) {
    color: var(--text-primary);
    font-size: 0.92rem;

    &::placeholder {
      color: var(--text-muted);
    }
  }

  :deep(.subcategory-select) {
    --el-fill-color-light: var(--bg-muted);
    --el-fill-color-blank: var(--bg-muted);
    --el-select-disabled-border: var(--border-subtle);
    --el-disabled-border-color: var(--border-subtle);
    --el-disabled-bg-color: var(--bg-muted);
    --el-disabled-text-color: var(--text-muted);
  }

  :deep(.subcategory-select .el-input.is-disabled .el-input__wrapper),
  :deep(.subcategory-select .el-input__wrapper.is-disabled),
  :deep(.subcategory-select .el-select__wrapper.is-disabled) {
    background: var(--bg-muted) !important;
    border-color: var(--border-subtle) !important;
    box-shadow: none !important;
    cursor: not-allowed;
    opacity: 0.72;
  }

  :deep(.subcategory-select .el-input.is-disabled .el-input__inner),
  :deep(.subcategory-select .el-input__inner:disabled),
  :deep(.subcategory-select .el-select__wrapper.is-disabled .el-select__selected-item),
  :deep(.subcategory-select .el-select__wrapper.is-disabled .el-select__placeholder) {
    -webkit-text-fill-color: var(--text-muted) !important;
    color: var(--text-muted) !important;
  }

  :deep(.subcategory-select .el-select__caret),
  :deep(.subcategory-select .el-select__wrapper.is-disabled .el-select__caret) {
    color: var(--text-muted) !important;
  }

  :deep(.el-input__count) {
    background: transparent;
    color: var(--text-muted);
    font-size: 0.78rem;
  }
}
</style>
