<template>
  <svg
    class="brand-mark"
    viewBox="0 0 88 88"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    role="img"
    :aria-hidden="decorative"
    :aria-label="decorative ? undefined : '萤火集标识'"
  >
    <path
      class="brand-mark__trail"
      d="M23 59C23 43 32 30 45 24C52 20.8 58.8 21.8 64 27"
    />
    <circle class="brand-mark__core" cx="61" cy="30" r="9" />
  </svg>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    decorative?: boolean;
  }>(),
  {
    decorative: true,
  },
);
</script>

<style scoped lang="scss">
.brand-mark {
  display: block;
  width: 100%;
  height: 100%;
  overflow: visible;
}

.brand-mark__trail {
  fill: none;
  stroke: var(--brand-primary, #6d84b3);
  stroke-width: 6;
  stroke-linecap: round;
  stroke-linejoin: round;
  opacity: 0.78;
}

.brand-mark__core {
  fill: var(--brand-accent, #be8a46);
}
</style>
