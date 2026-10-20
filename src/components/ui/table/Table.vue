<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { ScrollArea, ScrollBar } from '@/components/ui/scroll-area'
import { cn } from '@/lib/utils'

const props = defineProps<{
  class?: HTMLAttributes['class']
}>()
</script>

<template>
  <ScrollArea data-slot="table-container" class="relative w-full overflow-hidden rounded-[var(--radius)] border border-border">
    <table data-slot="table" :class="cn('w-full caption-bottom text-sm', props.class)">
      <slot />
    </table>
    <ScrollBar orientation="horizontal" />
  </ScrollArea>
</template>

