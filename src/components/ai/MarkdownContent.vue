<template>
  <div class="markdown-content" v-html="html"></div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import DOMPurify from "dompurify";
import { marked } from "marked";

const props = defineProps<{
  content: string;
}>();

marked.setOptions({
  breaks: true,
  gfm: true,
});

const html = computed(() => {
  const raw = marked.parse(props.content || "", {
    async: false,
  }) as string;
  return DOMPurify.sanitize(raw, {
    USE_PROFILES: { html: true },
  });
});
</script>

<style scoped lang="scss">
.markdown-content {
  color: inherit;
  font-size: 14px;
  line-height: 1.7;
  overflow-wrap: anywhere;
}

.markdown-content :deep(*) {
  letter-spacing: 0;
}

.markdown-content :deep(p),
.markdown-content :deep(ul),
.markdown-content :deep(ol),
.markdown-content :deep(blockquote),
.markdown-content :deep(pre),
.markdown-content :deep(table) {
  margin: 0 0 10px;
}

.markdown-content :deep(*:last-child) {
  margin-bottom: 0;
}

.markdown-content :deep(ul),
.markdown-content :deep(ol) {
  padding-left: 1.25em;
}

.markdown-content :deep(li + li) {
  margin-top: 4px;
}

.markdown-content :deep(strong) {
  color: var(--color-text-heading);
  font-weight: 800;
}

.markdown-content :deep(a) {
  color: var(--color-primary);
  font-weight: 700;
  text-decoration: none;
}

.markdown-content :deep(a:hover) {
  text-decoration: underline;
}

.markdown-content :deep(code) {
  border: 1px solid var(--stroke-soft);
  border-radius: 6px;
  padding: 1px 5px;
  background: var(--surface-card);
  font-family: "Cascadia Code", "Fira Code", Consolas, monospace;
  font-size: 0.92em;
}

.markdown-content :deep(pre) {
  overflow: auto;
  border: 1px solid var(--stroke-soft);
  border-radius: 8px;
  padding: 10px;
  background: var(--surface-card);
}

.markdown-content :deep(pre code) {
  border: none;
  padding: 0;
  background: transparent;
}

.markdown-content :deep(blockquote) {
  border-left: 3px solid var(--color-primary);
  padding-left: 10px;
  color: var(--color-text-secondary);
}

.markdown-content :deep(table) {
  width: 100%;
  border-collapse: collapse;
  display: block;
  overflow-x: auto;
}

.markdown-content :deep(th),
.markdown-content :deep(td) {
  border: 1px solid var(--stroke-soft);
  padding: 6px 8px;
  text-align: left;
}

.markdown-content :deep(th) {
  background: var(--surface-card);
  color: var(--color-text-heading);
}
</style>
