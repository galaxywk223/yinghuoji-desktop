<!-- 空状态组件 -->
<template>
  <div class="records-empty-workbench empty-state">
    <section class="empty-main">
      <div class="empty-icon">
        <Icon icon="lucide:notebook-pen" />
      </div>
      <h3>还没有任何记录</h3>
      <p class="text-muted">
        创建第一条学习日志后，周视图、分类统计和趋势分析会自动开始积累。
      </p>
      <el-button
        type="primary"
        size="large"
        class="mt-3"
        @click="$emit('add-record')"
      >
        <Icon icon="lucide:plus" />
        创建第一条记录
      </el-button>
    </section>

    <aside class="empty-guide">
      <div>
        <span class="guide-index">01</span>
        <p>选择当前学习阶段，记录会自动归入阶段时间线。</p>
      </div>
      <div>
        <span class="guide-index">02</span>
        <p>填写分类与子分类，后续图表会按分类汇总。</p>
      </div>
      <div>
        <span class="guide-index">03</span>
        <p>补充笔记，复盘时可以直接回看当时的上下文。</p>
      </div>
    </aside>
  </div>
</template>

<script setup>
import { Icon } from "@iconify/vue";

// Emits
defineEmits(["add-record"]);
</script>

<style scoped lang="scss">
.empty-state {
  min-height: max(420px, calc(100vh - var(--topbar-height) - 220px));
  padding: 28px;
  background: var(--bg-surface);
  border-radius: var(--radius-lg);
  border: 1px solid var(--border-subtle);
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(260px, 0.36fr);
  gap: 24px;
  align-items: stretch;

  .empty-icon {
    width: 64px;
    height: 64px;
    margin-bottom: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-lg);
    color: var(--brand-primary);
    background: var(--brand-primary-soft);

    :deep(svg) {
      width: 30px;
      height: 30px;
    }
  }

  h3 {
    color: var(--text-primary);
    margin: 0 0 12px;
    font-size: 22px;
    font-weight: 700;
  }

  .text-muted {
    max-width: 460px;
    color: var(--text-secondary);
    margin: 0 0 28px 0;
    font-size: 15px;
    line-height: 1.6;
  }

  .mt-3 {
    margin-top: 20px;
  }

  .el-button {
    height: 44px;
    padding: 0 28px;
    font-size: 14px;
    font-weight: 700;

    :deep(.iconify) {
      width: 18px;
      height: 18px;
      margin-right: 8px;
    }
  }
}

.empty-main {
  min-width: 0;
  padding: 28px;
  border-radius: var(--radius-lg);
  background: var(--bg-muted);
  display: flex;
  flex-direction: column;
  justify-content: center;
}

.empty-guide {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.empty-guide div {
  flex: 1;
  padding: 16px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-subtle);
  background: var(--bg-muted);
}

.guide-index {
  display: block;
  margin-bottom: 12px;
  color: var(--brand-primary);
  font-size: 12px;
  font-weight: 800;
}

.empty-guide p {
  margin: 0;
  color: var(--text-secondary);
  font-size: 13px;
  line-height: 1.6;
}

@media (max-width: 800px) {
  .empty-state {
    grid-template-columns: 1fr;
    min-height: 0;
  }
}
</style>
