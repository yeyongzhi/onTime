<script setup lang="ts">
import { computed } from 'vue'
import { Button } from '@/components/ui/button'
const page = defineModel<number>({ required: true })
const props = defineProps<{ total: number; pageSize: number; disabled?: boolean }>()
const pages = computed(() => Math.max(1, Math.ceil(props.total / props.pageSize)))
</script>
<template>
  <nav data-slot="card-footer" aria-label="列表分页" class="flex shrink-0 items-center justify-end gap-3 border-t px-5 py-3">
    <span class="text-sm text-muted-foreground">共 {{ total }} 项 · 每页 {{ pageSize }} 项 · 第 {{ page }} / {{ pages }} 页</span>
    <Button variant="secondary" size="sm" :disabled="disabled || page <= 1" @click="page--">上一页</Button>
    <Button variant="secondary" size="sm" :disabled="disabled || page >= pages" @click="page++">下一页</Button>
  </nav>
</template>
