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
      class="brand-mark__orbit"
      d="M44 13.5L74.5 44L44 74.5L13.5 44L44 13.5Z"
    />
    <rect
      class="brand-mark__node brand-mark__node--north"
      x="34.5"
      y="9.5"
      width="19"
      height="19"
      rx="6.5"
      transform="rotate(45 44 19)"
    />
    <rect
      class="brand-mark__node brand-mark__node--east"
      x="59.5"
      y="34.5"
      width="19"
      height="19"
      rx="6.5"
      transform="rotate(45 69 44)"
    />
    <rect
      class="brand-mark__node brand-mark__node--south"
      x="34.5"
      y="59.5"
      width="19"
      height="19"
      rx="6.5"
      transform="rotate(45 44 69)"
    />
    <rect
      class="brand-mark__node brand-mark__node--west"
      x="9.5"
      y="34.5"
      width="19"
      height="19"
      rx="6.5"
      transform="rotate(45 19 44)"
    />
    <circle class="brand-mark__spark brand-mark__spark--core" cx="44" cy="44" r="7.5" />
    <circle class="brand-mark__spark brand-mark__spark--trail-large" cx="23" cy="26.5" r="4" />
    <circle class="brand-mark__spark brand-mark__spark--trail-small" cx="16.5" cy="19.5" r="2.75" />
    <circle class="brand-mark__spark brand-mark__spark--echo" cx="67" cy="61.5" r="3.25" />
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

.brand-mark__orbit {
  stroke: var(--brand-primary-soft, #dde5f2);
  stroke-width: 5.5;
  stroke-linejoin: round;
  stroke-linecap: round;
  opacity: 0.92;
}

.brand-mark__node {
  transform-box: fill-box;
  transform-origin: center;
}

.brand-mark__node--north {
  fill: var(--brand-primary-soft, #dde5f2);
}

.brand-mark__node--east {
  fill: var(--brand-primary, #6d84b3);
}

.brand-mark__node--south {
  fill: var(--brand-primary-strong, #526c99);
}

.brand-mark__node--west {
  fill: color-mix(in srgb, var(--brand-primary, #6d84b3) 78%, white);
}

.brand-mark__spark--core {
  fill: var(--brand-accent, #be8a46);
}

.brand-mark__spark--trail-large {
  fill: color-mix(in srgb, var(--brand-accent, #be8a46) 86%, white);
  opacity: 0.88;
}

.brand-mark__spark--trail-small {
  fill: var(--brand-primary-soft, #dde5f2);
  opacity: 0.9;
}

.brand-mark__spark--echo {
  fill: var(--brand-primary-soft, #dde5f2);
  opacity: 0.82;
}
</style>
