<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Select, SelectTrigger, SelectValue, SelectContent, SelectItem } from '@/components/ui/select'
import { Table, TableHeader, TableHead, TableRow, TableBody, TableCell } from '@/components/ui/table'
import { Card, CardContent } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Badge } from '@/components/ui/badge'
import { Empty, EmptyHeader, EmptyTitle, EmptyDescription } from '@/components/ui/empty'
import { dateLabel, statusLabels, type Occurrence } from '@/lib/types'
interface Page { items: Occurrence[]; total: number; page: number; pageSize: number }
const keyword = ref('')
const status = ref('all')
const period = ref('90')
const page = ref(1)
const result = ref<Page>({ items: [], total: 0, page: 1, pageSize: 20 })
const loading = ref(false)
const totalPages = computed(() => Math.max(1, Math.ceil(result.value.total / result.value.pageSize)))
let sequence = 0
async function load() {
  const current = ++sequence
  loading.value = true
  const days = Number(period.value)
  const to = Math.floor(Date.now() / 1000) + 86400
  const from = days === 0 ? null : to - days * 86400
  try {
    const data = await invoke<Page>('get_history', { filter: { page: page.value, pageSize: 20, keyword: keyword.value, status: status.value === 'all' ? '' : status.value, from, to } })
    if (current === sequence) result.value = data
  } catch (error) { if (current === sequence) toast.error(`读取提醒历史失败：${String(error)}`) }
  finally { if (current === sequence) loading.value = false }
}
watch([keyword, status, period], () => { page.value = 1; void load() })
watch(page, () => void load())
onMounted(() => void load())
</script>
<template>
  <main class="workspace-page history-page">
    <Card><CardContent class="flex flex-col gap-3 py-3">
      <div class="flex items-center justify-between gap-3"><h1 class="workspace-heading">提醒历史</h1><p class="text-sm text-muted-foreground">共 {{ result.total }} 条</p></div>
      <div class="flex flex-wrap items-center gap-3">
        <Input v-model="keyword" class="min-w-52 flex-1" placeholder="搜索提醒名称…" aria-label="搜索提醒名称" />
        <Select v-model="status"><SelectTrigger class="w-40"><SelectValue placeholder="全部状态" /></SelectTrigger><SelectContent><SelectItem value="all">全部状态</SelectItem><SelectItem value="dismissed">已停止</SelectItem><SelectItem value="auto_stopped">自动停止</SelectItem><SelectItem value="skipped_expired">过期未补响</SelectItem><SelectItem value="cancelled">已取消</SelectItem><SelectItem value="ringing">响铃中</SelectItem><SelectItem value="snoozed">贪睡中</SelectItem></SelectContent></Select>
        <Select v-model="period"><SelectTrigger class="w-36"><SelectValue placeholder="时间范围" /></SelectTrigger><SelectContent><SelectItem value="7">最近 7 天</SelectItem><SelectItem value="30">最近 30 天</SelectItem><SelectItem value="90">最近 90 天</SelectItem><SelectItem value="0">全部时间</SelectItem></SelectContent></Select>
      </div>
    </CardContent></Card>
    <Card class="flex min-h-0 flex-1 flex-col"><CardContent class="flex min-h-0 flex-1 flex-col p-0"><ScrollArea class="min-h-0 flex-1">
      <Table v-if="result.items.length"><TableHeader><TableRow><TableHead>提醒</TableHead><TableHead>计划时间</TableHead><TableHead>结果</TableHead><TableHead>状态</TableHead><TableHead class="text-right">贪睡</TableHead></TableRow></TableHeader><TableBody><TableRow v-for="item in result.items" :key="item.id"><TableCell class="font-medium">{{ item.title }}</TableCell><TableCell>{{ dateLabel(item.scheduledAt) }}</TableCell><TableCell>{{ item.reason || '—' }}</TableCell><TableCell><Badge variant="secondary">{{ statusLabels[item.status] || item.status }}</Badge></TableCell><TableCell class="text-right">{{ item.snoozeCount }} 次</TableCell></TableRow></TableBody></Table>
      <Empty v-else class="flex min-h-full flex-col justify-center"><EmptyHeader><EmptyTitle>{{ loading ? '正在读取历史…' : '没有符合条件的提醒记录' }}</EmptyTitle><EmptyDescription>调整筛选条件，或等待提醒产生处理记录。</EmptyDescription></EmptyHeader></Empty>
    </ScrollArea></CardContent></Card>
    <div class="flex items-center justify-end gap-3"><span class="text-sm text-muted-foreground">第 {{ page }} / {{ totalPages }} 页</span><Button variant="outline" size="sm" :disabled="page <= 1 || loading" @click="page--">上一页</Button><Button variant="outline" size="sm" :disabled="page >= totalPages || loading" @click="page++">下一页</Button></div>
  </main>
</template>

<style scoped>
.history-page {
  box-sizing: border-box;
  height: 100dvh;
  min-height: 0;
  gap: 14px;
}
</style>
