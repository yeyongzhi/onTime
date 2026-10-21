<script setup lang="ts">
import { onUnmounted, ref, shallowRef, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { Calendar } from '@/components/ui/calendar'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { type DateValue, getLocalTimeZone, today } from '@internationalized/date'
import { Input } from '@/components/ui/input'
import ListPagination from '@/components/ListPagination.vue'
import { Select, SelectTrigger, SelectValue, SelectContent, SelectGroup, SelectItem } from '@/components/ui/select'
import { Table, TableHeader, TableHead, TableRow, TableBody, TableCell } from '@/components/ui/table'
import { Card, CardContent } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Badge } from '@/components/ui/badge'
import { Empty, EmptyHeader, EmptyTitle, EmptyDescription } from '@/components/ui/empty'
import { useAlarmsStore } from '@/stores/alarms'
import { BellIcon, Clock3Icon, CircleCheckIcon, CircleXIcon, TriangleAlertIcon } from '@lucide/vue'
import { dateLabel, statusLabels, type Occurrence } from '@/lib/types'
interface Page { items: Occurrence[]; total: number; page: number; pageSize: number }
const store = useAlarmsStore()
function reminderType(item: Occurrence) {
  const kind = item.context?.kind
  return kind === 'timer' ? '倒计时' : kind === 'once' ? '单次提醒' : kind === 'repeat' ? '重复提醒' : '旧记录（类型未知）'
}
const historicalGroups = ref<{ id: string; name: string }[]>([])
const historicalTags = ref<{ id: string; name: string }[]>([])
async function loadFacets() {
  try { const value = await invoke<{ groups: {id:string;name:string}[]; tags: {id:string;name:string}[] }>('get_history_facets'); historicalGroups.value = value.groups; historicalTags.value = value.tags } catch { /* History request reports unavailable storage. */ }
}
const keyword = ref('')
const status = ref('all')
const group = ref('all')
const tag = ref('all')
const type = ref('all')
const start = shallowRef<DateValue>()
const end = shallowRef<DateValue>()
const startOpen = ref(false)
const endOpen = ref(false)
const page = ref(1)
const result = ref<Page>({ items: [], total: 0, page: 1, pageSize: 20 })
const loading = ref(false)
let sequence = 0
async function load() {
  const current = ++sequence
  loading.value = true
  const from = start.value ? start.value.toDate(getLocalTimeZone()).getTime() / 1000 : null
  const to = end.value ? end.value.add({ days: 1 }).toDate(getLocalTimeZone()).getTime() / 1000 : null
  try {
    const data = await invoke<Page>('get_history', { filter: { page: page.value, pageSize: 20, keyword: keyword.value, status: status.value === 'all' ? '' : status.value, from, to, alarmIds: null, criteria: { kind: type.value, group: group.value, tag: tag.value } } })
    if (current === sequence) {
      const last = Math.max(1, Math.ceil(data.total / data.pageSize))
      if (page.value > last) page.value = last
      else result.value = data
    }
  } catch (error) { if (current === sequence) toast.error(`读取提醒历史失败：${String(error)}`) }
  finally { if (current === sequence) loading.value = false }
}
// Coalesce filter and page updates into one request; ignore stale results on unmount.
let loadTimer: ReturnType<typeof setTimeout> | undefined
function scheduleLoad() {
  sequence++
  if (loadTimer) clearTimeout(loadTimer)
  loadTimer = setTimeout(() => { void load() }, 150)
}
watch([keyword, status, group, tag, type, start, end], () => { page.value = 1; scheduleLoad() })
watch(page, scheduleLoad)
watch(() => store.snapshot, snapshot => {
  if (snapshot) void loadFacets()
  scheduleLoad()
}, { immediate: true })
onUnmounted(() => { sequence++; if (loadTimer) clearTimeout(loadTimer) })
</script>
<template>
  <main class="workspace-page history-page">
    <Card><CardContent class="flex flex-col gap-3 py-3">
      <div class="flex items-center justify-between gap-3"><h1 class="workspace-heading">提醒历史</h1><p class="text-sm text-muted-foreground">共 {{ result.total }} 条</p></div>
      <div class="flex flex-wrap items-center gap-3">
        <Select v-model="group"><SelectTrigger class="w-32"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="all">全部分组</SelectItem><SelectItem value="ungrouped">未分组</SelectItem><SelectItem v-for="item in historicalGroups" :key="item.id" :value="item.id">{{ item.name }}</SelectItem></SelectGroup></SelectContent></Select>
        <Input v-model="keyword" class="min-w-52 flex-1" placeholder="搜索提醒名称…" aria-label="搜索提醒名称" />
        <Select v-model="status"><SelectTrigger class="w-40"><SelectValue placeholder="全部状态" /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="all">全部状态</SelectItem><SelectItem value="dismissed">已停止</SelectItem><SelectItem value="auto_stopped">自动停止</SelectItem><SelectItem value="skipped_expired">过期未补响</SelectItem><SelectItem value="cancelled">已取消</SelectItem><SelectItem value="ringing">响铃中</SelectItem><SelectItem value="snoozed">贪睡中</SelectItem></SelectGroup></SelectContent></Select>
        <Select v-model="tag"><SelectTrigger class="w-32"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="all">全部标签</SelectItem><SelectItem v-for="item in historicalTags" :key="item.id" :value="item.id">{{ item.name }}</SelectItem></SelectGroup></SelectContent></Select>
      </div><div class="flex flex-wrap items-center gap-3">
        <span class="text-sm">提醒类型</span><Select v-model="type"><SelectTrigger class="w-36"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="all">全部类型</SelectItem><SelectItem value="once">单次提醒</SelectItem><SelectItem value="repeat">重复提醒</SelectItem><SelectItem value="timer">倒计时</SelectItem></SelectGroup></SelectContent></Select>
        <span class="text-sm">提醒日期</span><Popover v-model:open="startOpen"><PopoverTrigger as-child><Button variant="secondary">{{ start?.toString() ?? '开始日期' }}</Button></PopoverTrigger><PopoverContent class="w-auto p-0"><Calendar v-model="start" :max-value="end" :default-placeholder="today(getLocalTimeZone())" locale="zh-CN" layout="month-and-year" @update:model-value="startOpen = false" /></PopoverContent></Popover><span>至</span><Popover v-model:open="endOpen"><PopoverTrigger as-child><Button variant="secondary">{{ end?.toString() ?? '结束日期' }}</Button></PopoverTrigger><PopoverContent class="w-auto p-0"><Calendar v-model="end" :min-value="start" :default-placeholder="start ?? today(getLocalTimeZone())" locale="zh-CN" layout="month-and-year" @update:model-value="endOpen = false" /></PopoverContent></Popover>
        <Button v-if="keyword || status !== 'all' || group !== 'all' || tag !== 'all' || type !== 'all' || start || end" variant="secondary" @click="keyword=''; status='all'; group='all'; tag='all'; type='all'; start=undefined; end=undefined">清空筛选</Button>
      </div>
    </CardContent></Card>
    <Card class="flex min-h-0 flex-1 flex-col"><CardContent class="flex min-h-0 flex-1 flex-col px-4"><ScrollArea class="min-h-0 flex-1">
      <Table v-if="result.items.length"><TableHeader><TableRow><TableHead>提醒</TableHead><TableHead>提醒类型</TableHead><TableHead>提醒时间</TableHead><TableHead>结果</TableHead><TableHead>状态</TableHead><TableHead>贪睡</TableHead></TableRow></TableHeader><TableBody><TableRow v-for="item in result.items" :key="item.id"><TableCell class="font-medium">{{ item.title }}</TableCell><TableCell>{{ reminderType(item) }}<span v-if="item.context?.inferred" class="block text-xs text-muted-foreground" title="旧版记录缺少分类快照，此信息依据升级时的提醒设置补全">升级时分类</span></TableCell><TableCell>{{ dateLabel(item.scheduledAt) }}</TableCell><TableCell>{{ item.reason || '—' }}</TableCell><TableCell><Badge :class="`history-status-${item.status}`" variant="secondary"><component :is="item.status === 'ringing' ? BellIcon : item.status === 'snoozed' ? Clock3Icon : item.status === 'skipped_expired' ? TriangleAlertIcon : item.status === 'cancelled' ? CircleXIcon : CircleCheckIcon" data-icon="inline-start" />{{ statusLabels[item.status] || item.status }}</Badge></TableCell><TableCell>{{ item.snoozeCount }} 次</TableCell></TableRow></TableBody></Table>
      <Empty v-else class="flex min-h-full flex-col justify-center"><EmptyHeader><EmptyTitle>{{ loading ? '正在读取历史…' : '没有符合条件的提醒记录' }}</EmptyTitle><EmptyDescription>调整筛选条件，或等待提醒产生处理记录。</EmptyDescription></EmptyHeader></Empty>
    </ScrollArea></CardContent><ListPagination v-model="page" :total="result.total" :page-size="result.pageSize" :disabled="loading" /></Card>
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
