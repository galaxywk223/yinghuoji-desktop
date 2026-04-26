<template>
  <div class="theme-switcher">
    <el-popover
      placement="bottom-end"
      :width="320"
      trigger="click"
      popper-class="theme-popper"
    >
      <template #reference>
        <button type="button" class="theme-trigger" aria-label="切换主题">
          <Icon icon="lucide:palette" />
        </button>
      </template>

      <div class="theme-panel">
        <div class="theme-panel__header">
          <p class="app-kicker">Theme</p>
          <h3>工作台外观</h3>
          <p>选择色系，并在浅色与深色之间切换。</p>
        </div>

        <div class="theme-list">
          <section
            v-for="family in themeFamilies"
            :key="family.id"
            class="theme-family"
          >
            <span class="theme-family__copy">
              <strong>{{ family.name }}</strong>
              <small>{{ family.description }}</small>
            </span>
            <div class="theme-family__modes">
              <button
                v-for="theme in family.themes"
                :key="theme.id"
                type="button"
                class="theme-option"
                :class="{ active: currentTheme === theme.id }"
                :title="theme.description"
                @click="setTheme(theme.id)"
              >
                <span
                  class="theme-option__preview"
                  :style="{ background: theme.preview }"
                />
                <span>{{ theme.shortName }}</span>
                <Icon
                  v-if="currentTheme === theme.id"
                  icon="lucide:check"
                  class="theme-option__check"
                />
              </button>
            </div>
          </section>
        </div>
      </div>
    </el-popover>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { Icon } from "@iconify/vue";
import { useThemeStore } from "@/stores/modules/theme";

const themeStore = useThemeStore();

const currentTheme = computed(() => themeStore.currentTheme);
const themeFamilies = computed(() => themeStore.themeFamilies);

const setTheme = (id: string) => themeStore.setTheme(id);
</script>

<style scoped lang="scss">
.theme-trigger {
  width: 36px;
  height: 36px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-surface);
  color: var(--text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  box-shadow: var(--shadow-1);
}

.theme-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.theme-panel__header {
  display: flex;
  flex-direction: column;
  gap: 4px;

  h3,
  p {
    margin: 0;
  }

  h3 {
    color: var(--text-primary);
    font-size: 1rem;
  }

  p:last-child {
    color: var(--text-secondary);
    font-size: 0.84rem;
  }
}

.theme-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.theme-family {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  padding: 10px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  background: var(--bg-surface);
}

.theme-family__copy {
  display: flex;
  flex-direction: column;
  min-width: 0;

  strong {
    color: var(--text-primary);
    font-size: 0.9rem;
  }

  small {
    color: var(--text-muted);
    font-size: 0.76rem;
  }
}

.theme-family__modes {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.theme-option {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 7px;
  min-height: 34px;
  padding: 4px 9px 4px 5px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  color: var(--text-secondary);
  cursor: pointer;
  font-size: 0.8rem;
  font-weight: 700;
}

.theme-option.active {
  border-color: color-mix(in srgb, var(--brand-primary) 42%, var(--border-subtle));
  background: var(--brand-primary-soft);
  color: var(--brand-primary-strong);
}

.theme-option__preview {
  width: 22px;
  height: 22px;
  border-radius: 6px;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.34);
}

.theme-option__check {
  color: var(--brand-primary);
  width: 14px;
  height: 14px;
}
</style>
