<script setup lang="ts">
// Horizontal bars on a linear scale from zero: the short docker-exporter bar is the
// point, so no log scale. The markdown table next to each chart is the accessible view.
import { computed } from "vue";

const props = defineProps<{
  title: string;
  unit: string;
  digits?: number;
  rows: { label: string; value: number; highlight?: boolean }[];
}>();

const max = computed(() => Math.max(...props.rows.map((r) => r.value)));
const fmt = (v: number) =>
  `${v.toLocaleString("en-US", { minimumFractionDigits: props.digits ?? 1, maximumFractionDigits: props.digits ?? 1 })}${props.unit}`;
</script>

<template>
  <figure class="fp-chart">
    <figcaption>{{ title }}</figcaption>
    <div
      v-for="row in rows"
      :key="row.label"
      class="fp-row"
      :class="{ 'fp-hl': row.highlight }"
      :title="`${row.label}: ${fmt(row.value)}`"
    >
      <span class="fp-label">{{ row.label }}</span>
      <span class="fp-track">
        <span class="fp-bar" :style="{ width: `${(row.value / max) * 82}%` }" />
        <span class="fp-value">{{ fmt(row.value) }}</span>
      </span>
    </div>
  </figure>
</template>

<style scoped>
.fp-chart {
  /* Emphasis pair, checked with a CVD validator: lightness-separated so protan readers
     still tell docker-exporter from cAdvisor; value labels carry identity too. */
  --fp-hl: #0f766e;
  --fp-base: #a8a8ad;
  margin: 20px 0 8px;
}
:global(.dark .fp-chart) {
  --fp-hl: #2dd4bf;
  --fp-base: #6a6a70;
}
.fp-chart figcaption {
  font-size: 14px;
  font-weight: 600;
  color: var(--vp-c-text-1);
  margin-bottom: 10px;
}
.fp-row {
  display: grid;
  grid-template-columns: minmax(9rem, 15rem) 1fr;
  align-items: center;
  gap: 4px 12px;
  padding: 3px 0;
}
.fp-label {
  font-size: 13px;
  color: var(--vp-c-text-2);
  line-height: 1.3;
}
.fp-hl .fp-label {
  color: var(--vp-c-text-1);
  font-weight: 600;
}
.fp-track {
  display: flex;
  align-items: center;
  gap: 8px;
  border-left: 1px solid var(--vp-c-divider);
  min-height: 22px;
}
.fp-bar {
  display: block;
  height: 14px;
  min-width: 2px;
  border-radius: 0 4px 4px 0;
  background: var(--fp-base);
}
.fp-hl .fp-bar {
  background: var(--fp-hl);
}
.fp-value {
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  color: var(--vp-c-text-2);
  white-space: nowrap;
}
.fp-hl .fp-value {
  color: var(--vp-c-text-1);
  font-weight: 600;
}
@media (max-width: 640px) {
  .fp-row {
    grid-template-columns: 1fr;
    padding: 5px 0;
  }
}
</style>
