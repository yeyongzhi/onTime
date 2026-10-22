<script setup lang="ts">
import { computed, ref, shallowRef, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { appearance } from '@/stores/appearance'
import { toast } from 'vue-sonner'
import AlarmScheduleSummary from '@/components/AlarmScheduleSummary.vue'
import BrandLogo from '@/components/BrandLogo.vue'
import { AlarmClockIcon, PlusIcon, Trash2Icon, PencilIcon, TagIcon, ChevronDownIcon, LayoutGridIcon, ListIcon, XIcon, EllipsisIcon } from '@lucide/vue'
import type { Alarm } from '@/lib/types'
import { dateLabel, alarmEffective } from '@/lib/types'
import { getEffectivenessDisplay, getExpirationReason } from '@/lib/dictionaries'
import { useAlarmsStore } from '@/stores/alarms'
import { useRouter, useRoute } from 'vue-router'
import { ButtonGroup } from '@/components/ui/button-group'
import ListPagination from '@/components/ListPagination.vue'
import { Table, TableHeader, TableHead, TableRow, TableBody, TableCell } from '@/components/ui/table'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { Calendar } from '@/components/ui/calendar'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { type DateValue, today, getLocalTimeZone } from '@internationalized/date'
import { Badge } from '@/components/ui/badge'
import { InputGroup, InputGroupInput, InputGroupAddon, InputGroupButton } from '@/components/ui/input-group'
import { Checkbox } from '@/components/ui/checkbox'
import { Switch } from '@/components/ui/switch'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from '@/components/ui/card'
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle, EmptyDescription, EmptyContent } from '@/components/ui/empty'
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { cn } from '@/lib/utils'
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem, DropdownMenuCheckboxItem } from '@/components/ui/dropdown-menu'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { FieldLabel } from '@/components/ui/field'
import { Select, SelectTrigger, SelectValue, SelectContent, SelectGroup, SelectItem } from '@/components/ui/select'
const props=withDefaults(defineProps<{ management?: boolean }>(), { management: false })
const store=useAlarmsStore()
const router=useRouter()
const route=useRoute()
const groupFilter=ref(String(route.query.group ?? 'all'))
const expiredAlarm=ref<Alarm|null>(null)
const scoped=computed(() => store.snapshot?.alarms.filter(a => !props.management || groupFilter.value === 'all' || (groupFilter.value === 'ungrouped' ? !a.groupId : a.groupId === groupFilter.value)) ?? [])
function createReminder() {
  const query: Record<string, string> = { from: route.fullPath }
  if (!['all', 'ungrouped'].includes(groupFilter.value)) query.group = groupFilter.value
  router.push({ path: '/reminders/new', query })
}
const displayMode = computed({ get: () => appearance.displayMode, set: value => { appearance.displayMode = value as 'grid' | 'list' } })
const managing = ref(false)
const selectedIds = ref<string[]>([])
const batchDelete = ref(false)
const deletingBatch = ref(false)
const query=ref(''); const tagFilter=ref<string[]>(route.query.tag ? [String(route.query.tag)] : []); const deletion=ref<Alarm | null>(null)
const typeFilter = ref('all')
const dateStart = shallowRef<DateValue>()
const dateEnd = shallowRef<DateValue>()
const startOpen = ref(false)
const endOpen = ref(false)
const dateMatches = ref<string[]>([])
let dateSequence = 0
watch([dateStart, dateEnd, () => store.snapshot], async () => {
  const sequence = ++dateSequence
  if (!dateStart.value && !dateEnd.value) { dateMatches.value = []; return }
  const from = dateStart.value ? dateStart.value.toDate(getLocalTimeZone()).getTime() / 1000 : 0
  const to = dateEnd.value ? dateEnd.value.add({ days: 1 }).toDate(getLocalTimeZone()).getTime() / 1000 : 253402214400
  try { const ids = await invoke<string[]>('matching_alarm_dates', { from, to }); if (sequence === dateSequence) dateMatches.value = ids } catch (e) { if (sequence === dateSequence) { dateMatches.value = []; toast.error(String(e)) } }
})
function matchesDate(a: Alarm) { return (!dateStart.value && !dateEnd.value) || dateMatches.value.includes(a.id) }
const activities = computed(() => store.snapshot?.occurrences.filter(o => ['ringing', 'snoozed'].includes(o.status)) ?? [])
async function showActivities() { try { await invoke('show_active_alert') } catch (e) { toast.error(String(e)) } }
const filtered=computed(() => scoped.value.filter(a => !props.management || (
  a.title.toLowerCase().includes(query.value.toLowerCase()) &&
  (tagFilter.value.length === 0 || tagFilter.value.some(id => a.tags.includes(id))) &&
  (typeFilter.value === 'all' || (a.kind === 'timer' ? 'timer' : a.repeat === 'once' ? 'once' : 'repeat') === typeFilter.value) && matchesDate(a)
)))
watch([typeFilter, dateStart, dateEnd], () => { page.value = 1; selectedIds.value = [] })

watch(() => route.query, value => { groupFilter.value=String(value.group ?? 'all'); tagFilter.value=value.tag ? [String(value.tag)] : []; selectedIds.value=[] })
watch(() => props.management, () => { typeFilter.value='all'; dateStart.value=undefined; dateEnd.value=undefined; query.value=''; groupFilter.value=String(route.query.group ?? 'all'); tagFilter.value=route.query.tag ? [String(route.query.tag)] : []; finishManaging(); batchDelete.value=false; deletion.value=null; expiredAlarm.value=null })
watch(groupFilter, () => { selectedIds.value=[]; managing.value=false })
watch(() => store.snapshot, snapshot => {
  if (!snapshot) return
  if (!['all','ungrouped'].includes(groupFilter.value) && !snapshot.groups.some(g => g.id === groupFilter.value)) groupFilter.value='all'
  const validTags = tagFilter.value.filter(id => snapshot.tags.some(t => t.id === id))
  if (validTags.length !== tagFilter.value.length) tagFilter.value = validTags
})
const selectedVisible = computed(() => filtered.value.filter(a => selectedIds.value.includes(a.id)).map(a => a.id))
function selectAlarm(id: string, checked: boolean | 'indeterminate') { selectedIds.value = checked === true ? [...new Set([...selectedIds.value, id])] : selectedIds.value.filter(value => value !== id) }
function finishManaging() { managing.value = false; selectedIds.value = [] }
async function removeBatch() {
  const ids = [...selectedVisible.value]
  deletingBatch.value = true
  let deleted = 0
  try {
    await store.action('delete_alarms', { ids })
    selectedIds.value = selectedIds.value.filter(id => !ids.includes(id))
    deleted = ids.length
    batchDelete.value = false
    toast.success(`已删除 ${deleted} 个提醒`)
  } catch { /* Keep remaining selections available to retry. */ }
  finally { deletingBatch.value = false }
}
const homeSummary = ref<{ upcoming: number; reminders: number; processed: number } | null>(null)
const summaryError = ref(false)
let summarySequence = 0
watch([() => store.snapshot, () => Math.floor(store.now / 60), () => props.management], async () => {
  if (props.management) return
  const sequence = ++summarySequence
  if (!store.snapshot) { homeSummary.value = null; return }
  const start = new Date(store.now * 1000); start.setHours(0, 0, 0, 0)
  const end = new Date(start); end.setDate(end.getDate() + 1)
  const week = new Date(store.now * 1000); week.setDate(week.getDate() + 7)
  summaryError.value = false
  try {
    const result = await invoke<{ upcoming: number; reminders: number; processed: number }>('get_home_summary', { now: store.now, todayStart: start.getTime() / 1000, todayEnd: end.getTime() / 1000, weekEnd: Math.floor(week.getTime() / 1000) })
    if (sequence === summarySequence) homeSummary.value = result
  } catch { if (sequence === summarySequence) { homeSummary.value = null; summaryError.value = true } }
}, { immediate: true })
const todayPending = computed(() => {
  const end = new Date(store.now * 1000)
  end.setHours(24, 0, 0, 0)
  return (store.snapshot?.alarms.filter(a => {
    const time = store.snapshot?.nextRuns[a.id]
    return a.enabled && time !== undefined && time >= store.now && time < end.getTime() / 1000
  }).length ?? 0) + activities.value.filter(o => o.status === 'snoozed' && o.snoozeUntil && o.snoozeUntil >= store.now && o.snoozeUntil < end.getTime() / 1000).length
})
const nextRun = computed(() => {
  const snapshot = store.snapshot
  if (!snapshot) return null
  const next = snapshot.alarms.filter(a => a.enabled).map(a => ({ alarm: a, time: snapshot.nextRuns[a.id] })).filter((entry): entry is { alarm: Alarm; time: number } => entry.time !== undefined && entry.time >= store.now).sort((a,b) => a.time-b.time)[0]
  const snoozed = activities.value.filter(o => o.status === 'snoozed' && o.snoozeUntil && o.snoozeUntil >= store.now).map(o => ({ alarm: snapshot.alarms.find(a => a.id === o.alarmId)!, time: o.snoozeUntil! })).filter(o => o.alarm).sort((a,b) => a.time-b.time)[0]
  return snoozed && (!next || snoozed.time < next.time) ? snoozed : next ?? null
})
const nextCountdown = computed(() => {
  if (!nextRun.value) return '暂无安排'
  const seconds = Math.max(0, nextRun.value.time - store.now)
  const days = Math.floor(seconds / 86400)
  const clock = [Math.floor(seconds % 86400 / 3600), Math.floor(seconds % 3600 / 60), seconds % 60].map(value => String(value).padStart(2, '0')).join(':')
  return days ? `${days}天 ${clock}` : clock
})
const recentMissed = computed(() => store.snapshot?.occurrences
  .filter(occurrence => occurrence.status === 'skipped_expired')
  .sort((a, b) => b.scheduledAt - a.scheduledAt)
  .slice(0, 3) ?? [])
function editMissed(occurrenceId: string) {
  const occurrence = recentMissed.value.find(item => item.id === occurrenceId)
  if (!occurrence || !store.snapshot?.alarms.some(alarm => alarm.id === occurrence.alarmId)) return
  router.push({ name: 'edit-reminder', params: { id: occurrence.alarmId }, query: { from: route.fullPath } })
}
async function toggle(ids:string[],enabled:boolean) { const expired = enabled ? scoped.value.find(a => ids.includes(a.id) && !a.enabled && a.kind === 'clock' && a.repeat === 'once' && new Date(`${a.date}T${a.time}:00`).getTime() <= Date.now()) : null; if (expired) { expiredAlarm.value=expired; return } try { const message = ids.length > 1 ? `${ids.length} 个提醒已${enabled ? '启用' : '停用'}` : enabled ? '提醒已启用' : '提醒已停用'; await store.action('set_enabled',{ids,enabled},message) } catch { /* toast */ } }
async function chooseRoot() { try { const snapshot = await store.action('choose_data_root'); if (snapshot) toast.success('数据目录已连接') } catch { /* toast */ } }

async function remove() { if(!deletion.value) return; try { await store.action('delete_alarm',{id:deletion.value.id},'提醒已删除'); deletion.value=null } catch { /* toast */ } }
function clearSearch() { query.value=''; document.getElementById('reminder-search')?.focus() }
function filterTag(id: string, checked: boolean) { tagFilter.value = checked ? [...new Set([...tagFilter.value, id])] : tagFilter.value.filter(value => value !== id) }
const page = ref(1)
const pageSize = 10
const totalPages = computed(() => Math.max(1, Math.ceil(filtered.value.length / pageSize)))
const paged = computed(() => filtered.value.slice((page.value - 1) * pageSize, page.value * pageSize))
watch(totalPages, total => { page.value = Math.min(page.value, total) })
watch([query, tagFilter, groupFilter, () => props.management], () => { page.value = 1 })
</script>
<template>
  <main class="workspace-page reminder-page">
    <Alert v-if="activities.length" class="shrink-0"><AlertTitle>活动提醒 · {{ activities.length }}</AlertTitle><AlertDescription class="flex flex-wrap items-center gap-3"><span v-for="o in activities" :key="o.id">{{ o.title }} · {{ o.status === 'ringing' ? '响铃中' : `贪睡至 ${dateLabel(o.snoozeUntil!)}` }}</span><Button variant="secondary" @click="showActivities">查看活动提醒</Button></AlertDescription></Alert>
    <section v-if="!management" class="home-summary" aria-label="提醒概览">
      <Card><CardHeader><CardDescription>今日待提醒</CardDescription><CardTitle class="text-lg font-medium tabular-nums">{{ todayPending }}<span class="ml-2 text-sm text-muted-foreground">个提醒</span></CardTitle><CardDescription>今天剩余的提醒，含贪睡</CardDescription></CardHeader></Card>
      <Card><CardHeader><CardDescription>下一次提醒</CardDescription><CardTitle class="text-lg font-medium tabular-nums">{{ nextRun ? dateLabel(nextRun.time) : '暂无安排' }}</CardTitle><CardDescription class="truncate">{{ nextRun?.alarm.title ?? '启用提醒后显示时间' }}</CardDescription></CardHeader></Card>
      <Card><CardHeader><CardDescription>距离下次提醒</CardDescription><CardTitle class="text-lg font-medium tabular-nums">{{ nextCountdown }}</CardTitle><CardDescription>{{ nextRun ? '倒计时实时更新' : '启用提醒后开始倒计时' }}</CardDescription></CardHeader></Card>
      <Card><CardHeader><CardDescription>未来 7 天</CardDescription><CardTitle class="text-lg font-medium tabular-nums">{{ homeSummary?.upcoming ?? '—' }}<span class="ml-2 text-sm text-muted-foreground">次提醒</span></CardTitle><CardDescription>{{ summaryError ? '统计暂不可用' : `涉及 ${homeSummary?.reminders ?? 0} 个提醒，含贪睡` }}</CardDescription></CardHeader></Card>
      <Card><CardHeader><CardDescription>今日已处理</CardDescription><CardTitle class="text-lg font-medium tabular-nums">{{ homeSummary?.processed ?? '—' }}<span class="ml-2 text-sm text-muted-foreground">次提醒</span></CardTitle><CardDescription>{{ summaryError ? '统计暂不可用' : '今日计划中已停止或自动停止' }}</CardDescription></CardHeader></Card>
    </section>
    <Card v-else class="shrink-0"><CardContent v-if="management" class="flex flex-col gap-4">      <div v-if="management" class="flex min-w-0 items-center gap-2">
        <Select v-model="groupFilter"><SelectTrigger aria-label="筛选分组" class="w-32 shrink-0"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="all">全部分组</SelectItem><SelectItem value="ungrouped">未分组</SelectItem><SelectItem v-for="group in store.snapshot?.groups" :key="group.id" :value="group.id">{{ group.name }}</SelectItem></SelectGroup></SelectContent></Select>
        <InputGroup class="min-w-16 flex-1"><InputGroupInput id="reminder-search" v-model="query" placeholder="搜索提醒名称…" aria-label="搜索提醒" /><InputGroupAddon v-if="query.length" align="inline-end"><InputGroupButton size="icon-xs" aria-label="清空搜索" title="清空搜索" @click="clearSearch"><XIcon /></InputGroupButton></InputGroupAddon></InputGroup>
        <DropdownMenu><DropdownMenuTrigger as-child><Button variant="secondary"><TagIcon data-icon="inline-start" />{{ tagFilter.length ? `标签 · ${tagFilter.length}` : '全部标签' }}<ChevronDownIcon data-icon="inline-end" /></Button></DropdownMenuTrigger><DropdownMenuContent align="start"><DropdownMenuGroup><DropdownMenuCheckboxItem :model-value="tagFilter.length === 0" @update:model-value="checked => { if (checked) tagFilter = [] }" @select.prevent>全部标签</DropdownMenuCheckboxItem><DropdownMenuCheckboxItem v-for="tag in store.snapshot?.tags" :key="tag.id" :model-value="tagFilter.includes(tag.id)" @update:model-value="checked => filterTag(tag.id, checked)" @select.prevent><span class="size-2 rounded-full" :style="{ backgroundColor: tag.color }" />{{ tag.name }}</DropdownMenuCheckboxItem></DropdownMenuGroup></DropdownMenuContent></DropdownMenu>
        <div class="ml-auto flex shrink-0 items-center gap-2"><Tabs v-model="displayMode"><TabsList aria-label="提醒显示方式"><TabsTrigger value="grid"><LayoutGridIcon />网格</TabsTrigger><TabsTrigger value="list"><ListIcon />列表</TabsTrigger></TabsList></Tabs>
          <ButtonGroup aria-label="提醒操作"><Button @click="createReminder()" :disabled="!store.snapshot || !!store.error || store.busy"><PlusIcon data-icon="inline-start" />新建提醒</Button><DropdownMenu><DropdownMenuTrigger as-child><Button size="icon" aria-label="更多提醒操作" :disabled="!store.snapshot || deletingBatch"><EllipsisIcon /></Button></DropdownMenuTrigger><DropdownMenuContent align="end"><DropdownMenuGroup><DropdownMenuItem :disabled="store.busy" @select="managing ? finishManaging() : managing = true">{{ managing ? '完成管理' : '批量管理' }}</DropdownMenuItem></DropdownMenuGroup></DropdownMenuContent></DropdownMenu></ButtonGroup>
        </div>
      </div>
      <div class="flex flex-wrap items-center gap-3">
        <FieldLabel for="reminder-type-filter">提醒类型</FieldLabel><Select v-model="typeFilter"><SelectTrigger id="reminder-type-filter" class="w-36"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="all">全部类型</SelectItem><SelectItem value="once">单次提醒</SelectItem><SelectItem value="repeat">重复提醒</SelectItem><SelectItem value="timer">倒计时</SelectItem></SelectGroup></SelectContent></Select>
        <FieldLabel for="reminder-date-start">提醒日期</FieldLabel>
        <Popover v-model:open="startOpen"><PopoverTrigger as-child><Button id="reminder-date-start" variant="secondary">{{ dateStart?.toString() ?? '开始日期' }}</Button></PopoverTrigger><PopoverContent class="w-auto p-0" align="start"><Calendar v-model="dateStart" :max-value="dateEnd" :default-placeholder="today(getLocalTimeZone())" locale="zh-CN" layout="month-and-year" :week-starts-on="1" @update:model-value="startOpen = false" /></PopoverContent></Popover>
        <span class="text-muted-foreground">至</span>
        <Popover v-model:open="endOpen"><PopoverTrigger as-child><Button variant="secondary" aria-label="提醒结束日期">{{ dateEnd?.toString() ?? '结束日期' }}</Button></PopoverTrigger><PopoverContent class="w-auto p-0" align="start"><Calendar v-model="dateEnd" :min-value="dateStart" :default-placeholder="dateStart ?? today(getLocalTimeZone())" locale="zh-CN" layout="month-and-year" :week-starts-on="1" @update:model-value="endOpen = false" /></PopoverContent></Popover>
        <Button v-if="typeFilter !== 'all' || dateStart || dateEnd" variant="secondary" @click="typeFilter = 'all'; dateStart = undefined; dateEnd = undefined">清空筛选</Button>
      </div>
      <div v-if="management && managing" class="flex flex-wrap items-center gap-3" role="group" aria-label="批量管理">
        <Checkbox id="select-visible" :model-value="selectedVisible.length > 0 && selectedVisible.length === filtered.length ? true : selectedVisible.length ? 'indeterminate' : false" :disabled="deletingBatch || store.busy || !filtered.length" @update:model-value="value => selectedIds = value === true ? filtered.map(a => a.id) : []" /><FieldLabel for="select-visible">全选当前结果</FieldLabel><span class="text-sm text-muted-foreground">已选 {{ selectedVisible.length }} 项</span>
        <Button size="sm" :disabled="!selectedVisible.length || store.busy || deletingBatch" @click="toggle(selectedVisible, true)">启用</Button><Button size="sm" variant="secondary" :disabled="!selectedVisible.length || store.busy || deletingBatch" @click="toggle(selectedVisible, false)">停用</Button><Button size="sm" variant="destructive" :disabled="!selectedVisible.length || store.busy || deletingBatch" @click="batchDelete = true">删除</Button><Button class="ml-auto" size="sm" variant="secondary" :disabled="store.busy || deletingBatch" @click="finishManaging">退出管理</Button>
      </div>
</CardContent></Card>
    <Card class="reminder-content"><CardHeader v-if="!management" class="flex flex-row items-center justify-between gap-3"><CardTitle>我的提醒</CardTitle><Tabs v-model="displayMode"><TabsList aria-label="提醒显示方式"><TabsTrigger value="grid"><LayoutGridIcon />网格</TabsTrigger><TabsTrigger value="list"><ListIcon />列表</TabsTrigger></TabsList></Tabs></CardHeader><ScrollArea class="min-h-0 flex-1"><CardContent class="flex flex-col gap-6">
    <Alert v-if="store.error" variant="destructive"><AlertTitle>需要处理</AlertTitle><AlertDescription>{{ store.error }}<Button variant="secondary" size="sm" class="mt-2" @click="chooseRoot">重新选择数据目录</Button></AlertDescription></Alert>
    <p v-if="store.loading" role="status" class="text-muted-foreground">正在读取本地数据…</p>
    <Empty v-else-if="!store.snapshot"><EmptyHeader><EmptyMedia><BrandLogo class="size-14" /></EmptyMedia><EmptyTitle>欢迎使用 onTime</EmptyTitle><EmptyDescription>选择数据目录，开始创建提醒。</EmptyDescription></EmptyHeader><EmptyContent><Button @click="chooseRoot" :disabled="store.busy">选择数据目录</Button></EmptyContent></Empty>
    <template v-else>
      <Card v-if="!management && recentMissed.length" size="sm"><CardHeader><CardTitle>过期未补响</CardTitle><CardDescription>应用未运行或电脑休眠超过补响时限的提醒，最近 90 天最多显示 3 条。</CardDescription></CardHeader><CardContent class="flex flex-col gap-3"><div v-for="occurrence in recentMissed" :key="occurrence.id" class="flex flex-wrap items-center justify-between gap-3 rounded-md border p-3"><div class="min-w-0"><p class="truncate font-medium">{{ occurrence.title }}</p><p class="text-sm text-muted-foreground">计划 {{ dateLabel(occurrence.scheduledAt) }} · {{ occurrence.reason }}</p></div><Button v-if="store.snapshot.alarms.some(alarm => alarm.id === occurrence.alarmId)" variant="outline" size="sm" @click="editMissed(occurrence.id)">调整提醒</Button></div></CardContent></Card>
      <Table v-if="filtered.length && displayMode === 'list'"><TableHeader><TableRow><TableHead>序号</TableHead><TableHead>提醒名称</TableHead><TableHead>提醒类型</TableHead><TableHead>提醒时间</TableHead><TableHead>状态</TableHead><TableHead>失效原因</TableHead><TableHead>分组</TableHead><TableHead>标签</TableHead><TableHead>启用状态</TableHead><TableHead>操作</TableHead></TableRow></TableHeader><TableBody><TableRow v-for="(a, index) in paged" :key="a.id">
<TableCell class="tabular-nums">{{ (page - 1) * pageSize + index + 1 }}</TableCell><TableCell><div class="flex items-center gap-2"><Checkbox v-if="management && managing" :model-value="selectedIds.includes(a.id)" :aria-label="`选择提醒 ${a.title}`" :disabled="store.busy || deletingBatch" @update:model-value="v => selectAlarm(a.id, v)" /><div><p class="font-medium max-w-64 truncate" :title="a.title">{{ a.title }}</p></div></div></TableCell>
<TableCell>{{ a.kind === 'timer' ? '倒计时' : a.repeat === 'once' ? '单次提醒' : '重复提醒' }}</TableCell><TableCell class="reminder-schedule-cell"><AlarmScheduleSummary compact :alarm="a" :next-run="store.snapshot.nextRuns[a.id]" :now="store.now" /></TableCell><TableCell><Tooltip v-if="!alarmEffective(a, store.now)"><TooltipTrigger as-child><Badge :variant="getEffectivenessDisplay(a, store.now).variant" tabindex="0">{{ getEffectivenessDisplay(a, store.now).label }}</Badge></TooltipTrigger><TooltipContent>失效原因：{{ getExpirationReason(a, store.now, store.snapshot.occurrences) }}</TooltipContent></Tooltip><Badge v-else :variant="getEffectivenessDisplay(a, store.now).variant">{{ getEffectivenessDisplay(a, store.now).label }}</Badge></TableCell><TableCell class="whitespace-normal min-w-40">{{ getExpirationReason(a, store.now, store.snapshot.occurrences) ?? '—' }}</TableCell><TableCell>{{ store.snapshot.groups.find(g => g.id === a.groupId)?.name ?? '未分组' }}</TableCell><TableCell><div class="flex flex-wrap gap-1"><Badge v-for="tag in store.snapshot.tags.filter(t => a.tags.includes(t.id))" :key="tag.id" variant="outline"><span class="size-2 rounded-full" :style="{ backgroundColor: tag.color }" />{{ tag.name }}</Badge><span v-if="!a.tags.length">—</span></div></TableCell><TableCell><Badge :variant="a.enabled ? 'default' : 'outline'">{{ a.enabled ? '已启用' : '已停用' }}</Badge></TableCell>
<TableCell><div class="flex justify-start gap-2"><Button variant="secondary" size="sm" :disabled="store.busy" @click="toggle([a.id], !a.enabled)">{{ a.enabled ? '停用' : '启用' }}</Button><Button variant="secondary" size="sm" @click="router.push({name:'edit-reminder',params:{id:a.id},query:{from:route.fullPath}})">编辑</Button><Button variant="destructive" size="sm" :disabled="store.busy" @click="deletion=a">删除</Button></div></TableCell></TableRow></TableBody></Table>
      <div v-else-if="filtered.length" :class="cn('grid gap-4', displayMode === 'grid' ? 'reminder-grid' : 'reminder-list')"><Card v-for="a in paged" :key="a.id" class="reminder-card" :size="displayMode === 'grid' ? 'sm' : 'default'" :data-selected="managing && selectedIds.includes(a.id)"><CardHeader><div class="flex min-w-0 items-start justify-between gap-3"><div class="min-w-0 flex-1"><div class="flex min-w-0 flex-wrap items-center gap-2"><Checkbox v-if="management && managing" :model-value="selectedIds.includes(a.id)" :aria-label="`选择提醒 ${a.title}`" :disabled="deletingBatch || store.busy" @update:model-value="value => selectAlarm(a.id, value)" /><CardTitle class="group-data-[size=sm]/card:text-base font-bold line-clamp-2 break-words" :title="a.title">{{ a.title }}</CardTitle><Badge v-if="a.groupId" variant="secondary" class="text-sm h-auto"><span v-if="store.snapshot.groups.find(g => g.id === a.groupId)?.color" class="size-2 shrink-0 rounded-full" :style="{ backgroundColor: store.snapshot.groups.find(g => g.id === a.groupId)?.color }" />{{ store.snapshot.groups.find(g => g.id === a.groupId)?.name }}</Badge></div></div><Tooltip v-if="!alarmEffective(a, store.now)"><TooltipTrigger as-child><Badge :variant="getEffectivenessDisplay(a, store.now).variant" class="text-sm h-auto" tabindex="0">{{ getEffectivenessDisplay(a, store.now).label }}</Badge></TooltipTrigger><TooltipContent>失效原因：{{ getExpirationReason(a, store.now, store.snapshot.occurrences) }}</TooltipContent></Tooltip><Tooltip v-else><TooltipTrigger as-child><span class="inline-flex shrink-0"><Switch class="data-[state=unchecked]:bg-destructive dark:data-[state=unchecked]:bg-destructive" :model-value="a.enabled" :aria-label="`${a.title}：${a.enabled ? '已启用' : '已停用'}`" :disabled="store.busy" @update:model-value="v=>toggle([a.id],v)" /></span></TooltipTrigger><TooltipContent>{{ a.enabled ? '已启用' : '已停用' }}</TooltipContent></Tooltip></div></CardHeader><CardContent><AlarmScheduleSummary :alarm="a" :next-run="store.snapshot.nextRuns[a.id]" :now="store.now" /><div v-if="a.tags.length" class="mt-3 flex flex-wrap gap-2"><Badge v-for="tag in store.snapshot.tags.filter(t=>a.tags.includes(t.id))" :key="tag.id" variant="outline" class="text-sm h-auto"><span class="size-2 rounded-full" :style="{backgroundColor:tag.color}" />{{ tag.name }}</Badge></div></CardContent><CardFooter class="flex justify-between gap-2 px-3 py-2"><span class="text-sm text-muted-foreground">{{ a.kind === 'timer' ? '倒计时' : a.repeat === 'once' ? '单次提醒' : '重复提醒' }}</span><div v-if="management" class="flex items-center gap-2"><Switch v-if="displayMode === 'list'" :model-value="a.enabled" :aria-label="`${a.title}启用开关`" :disabled="store.busy" @update:model-value="v => toggle([a.id], v)" /><Tooltip v-if="management"><TooltipTrigger as-child><Button variant="secondary" size="icon-sm" aria-label="编辑提醒" @click="router.push({name:'edit-reminder',params:{id:a.id},query:{from:'/reminders'}})"><PencilIcon /></Button></TooltipTrigger><TooltipContent>编辑提醒</TooltipContent></Tooltip><Tooltip v-if="management"><TooltipTrigger as-child><Button variant="destructive" size="icon-sm" aria-label="删除提醒" @click="deletion=a"><Trash2Icon /></Button></TooltipTrigger><TooltipContent>删除提醒</TooltipContent></Tooltip></div></CardFooter></Card></div>
      <Empty v-else><EmptyHeader><EmptyMedia variant="icon"><AlarmClockIcon /></EmptyMedia><EmptyTitle>{{ store.snapshot.alarms.length?'没有匹配的提醒':'从第一个提醒开始' }}</EmptyTitle><EmptyDescription>{{ management ? '点击下方按钮创建提醒。' : '在“全部提醒”中创建和管理提醒。' }}</EmptyDescription></EmptyHeader><EmptyContent v-if="management"><Button variant="secondary" @click="createReminder()">新建提醒</Button></EmptyContent></Empty>
    </template>

    </CardContent></ScrollArea><ListPagination v-if="filtered.length" v-model="page" :total="filtered.length" :page-size="pageSize" /></Card>
    <Dialog v-model:open="batchDelete"><DialogContent @interact-outside="event => { if (deletingBatch) event.preventDefault() }" @escape-key-down="event => { if (deletingBatch) event.preventDefault() }"><DialogHeader><DialogTitle>删除选中的 {{ selectedVisible.length }} 个提醒？</DialogTitle><DialogDescription>将删除所选提醒并取消它们的活动提醒，历史记录保留。</DialogDescription></DialogHeader><DialogFooter><Button variant="secondary" :disabled="deletingBatch" @click="batchDelete = false">取消</Button><Button variant="destructive" :disabled="deletingBatch || store.busy" @click="removeBatch">{{ deletingBatch ? '正在删除…' : '确认删除' }}</Button></DialogFooter></DialogContent></Dialog>
    <Dialog :open="!!deletion" @update:open="v=>{if(!v) deletion=null}"><DialogContent><DialogHeader><DialogTitle>删除提醒？</DialogTitle><DialogDescription>将删除“{{ deletion?.title }}”并取消它的活动提醒，历史记录保留。</DialogDescription></DialogHeader><DialogFooter><Button variant="secondary" @click="deletion=null">取消</Button><Button variant="destructive" @click="remove" :disabled="store.busy">删除</Button></DialogFooter></DialogContent></Dialog>
    <Dialog :open="!!expiredAlarm" @update:open="v => { if (!v) expiredAlarm=null }"><DialogContent><DialogHeader><DialogTitle>请先调整提醒时间</DialogTitle><DialogDescription>“{{ expiredAlarm?.title }}”的一次性时间已过。请选择未来的日期和时间后保存启用。</DialogDescription></DialogHeader><DialogFooter><Button variant="secondary" @click="expiredAlarm=null">取消</Button><Button @click="router.push({name:'edit-reminder',params:{id:expiredAlarm!.id}})">编辑提醒</Button></DialogFooter></DialogContent></Dialog>
  </main>
</template>
