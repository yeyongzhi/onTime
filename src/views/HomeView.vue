<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import BrandLogo from '@/components/BrandLogo.vue'
import { AlarmClockIcon, PlusIcon, Trash2Icon, PencilIcon, TimerIcon, TagIcon, ChevronDownIcon, LayoutGridIcon, ListIcon, XIcon, EllipsisIcon } from '@lucide/vue'
import type { Alarm } from '@/lib/types'
import { repeatLabels, dateLabel } from '@/lib/types'
import { useAlarmsStore } from '@/stores/alarms'
import { useRouter, useRoute } from 'vue-router'
import { ButtonGroup } from '@/components/ui/button-group'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
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
const displayMode=ref('grid')
const managing = ref(false)
const selectedIds = ref<string[]>([])
const batchDelete = ref(false)
const deletingBatch = ref(false)
const query=ref(''); const tagFilter=ref<string[]>(route.query.tag ? [String(route.query.tag)] : []); const deletion=ref<Alarm | null>(null)
const filtered=computed(() => scoped.value.filter(a => !props.management || (a.title.toLowerCase().includes(query.value.toLowerCase()) && (tagFilter.value.length === 0 || tagFilter.value.some(id => a.tags.includes(id))))))
watch(() => route.query, value => { groupFilter.value=String(value.group ?? 'all'); tagFilter.value=value.tag ? [String(value.tag)] : []; selectedIds.value=[] })
watch(() => props.management, () => { query.value=''; groupFilter.value=String(route.query.group ?? 'all'); tagFilter.value=route.query.tag ? [String(route.query.tag)] : []; finishManaging(); batchDelete.value=false; deletion.value=null; expiredAlarm.value=null })
watch(groupFilter, () => { selectedIds.value=[]; managing.value=false })
watch(() => store.snapshot, snapshot => {
  if (!snapshot) return
  if (!['all','ungrouped'].includes(groupFilter.value) && !snapshot.groups.some(g => g.id === groupFilter.value)) groupFilter.value='all'
  tagFilter.value=tagFilter.value.filter(id => snapshot.tags.some(t => t.id === id))
})
const selectedVisible = computed(() => filtered.value.filter(a => selectedIds.value.includes(a.id)).map(a => a.id))
function selectAlarm(id: string, checked: boolean | 'indeterminate') { selectedIds.value = checked === true ? [...new Set([...selectedIds.value, id])] : selectedIds.value.filter(value => value !== id) }
function finishManaging() { managing.value = false; selectedIds.value = [] }
async function removeBatch() {
  const ids = [...selectedVisible.value]
  deletingBatch.value = true
  let deleted = 0
  try {
    for (const id of ids) {
      await store.action('delete_alarm', { id })
      selectedIds.value = selectedIds.value.filter(value => value !== id)
      deleted++
    }
    batchDelete.value = false
    toast.success(`已删除 ${deleted} 个提醒`)
  } catch { /* Keep remaining selections available to retry. */ }
  finally { deletingBatch.value = false }
}
const todayPending = computed(() => {
  const end = new Date(store.now * 1000)
  end.setHours(24, 0, 0, 0)
  return store.snapshot?.alarms.filter(a => {
    const time = store.snapshot?.nextRuns[a.id]
    return a.enabled && time !== undefined && time >= store.now && time < end.getTime() / 1000
  }).length ?? 0
})
const nextRun = computed(() => {
  const snapshot = store.snapshot
  if (!snapshot) return null
  const next = snapshot.alarms.filter(a => a.enabled).map(a => ({ alarm: a, time: snapshot.nextRuns[a.id] })).filter((entry): entry is { alarm: Alarm; time: number } => entry.time !== undefined && entry.time >= store.now).sort((a,b) => a.time-b.time)[0]
  return next ?? null
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
function remaining(a:Alarm) { const t=store.snapshot?.nextRuns[a.id]; if(!t) return a.enabled ? '本次已完成' : '已停用'; if(a.kind==='clock') return dateLabel(t); const d=Math.max(0,t-store.now); return `${Math.floor(d/3600)}:${String(Math.floor(d%3600/60)).padStart(2,'0')}:${String(d%60).padStart(2,'0')}` }
async function toggle(ids:string[],enabled:boolean) { const expired = enabled ? scoped.value.find(a => ids.includes(a.id) && !a.enabled && a.kind === 'clock' && a.repeat === 'once' && new Date(`${a.date}T${a.time}:00`).getTime() <= Date.now()) : null; if (expired) { expiredAlarm.value=expired; return } try { const message = ids.length > 1 ? `${ids.length} 个提醒已${enabled ? '启用' : '停用'}` : enabled ? '提醒已启用' : '提醒已停用'; await store.action('set_enabled',{ids,enabled},message) } catch { /* toast */ } }
async function chooseRoot() { try { const snapshot = await store.action('choose_data_root'); if (snapshot) toast.success('数据目录已连接') } catch { /* toast */ } }

async function remove() { if(!deletion.value) return; try { await store.action('delete_alarm',{id:deletion.value.id},'提醒已删除'); deletion.value=null } catch { /* toast */ } }
function clearSearch() { query.value=''; document.getElementById('reminder-search')?.focus() }
function filterTag(id: string, checked: boolean) { tagFilter.value = checked ? [...new Set([...tagFilter.value, id])] : tagFilter.value.filter(value => value !== id) }
</script>
<template>
  <main class="workspace-page reminder-page">
    <section v-if="!management" class="home-summary" aria-label="提醒概览">
      <Card><CardHeader><CardDescription>今日待提醒</CardDescription><CardTitle class="text-2xl tabular-nums">{{ todayPending }}<span class="ml-2 text-sm text-muted-foreground">个提醒</span></CardTitle><CardDescription>今天剩余的提醒</CardDescription></CardHeader></Card>
      <Card><CardHeader><CardDescription>下一次提醒</CardDescription><CardTitle class="text-xl tabular-nums">{{ nextRun ? dateLabel(nextRun.time) : '暂无安排' }}</CardTitle><CardDescription class="truncate">{{ nextRun?.alarm.title ?? '启用提醒后显示时间' }}</CardDescription></CardHeader></Card>
      <Card><CardHeader><CardDescription>距离下次提醒</CardDescription><CardTitle class="text-2xl tabular-nums">{{ nextCountdown }}</CardTitle><CardDescription>{{ nextRun ? '倒计时实时更新' : '启用提醒后开始倒计时' }}</CardDescription></CardHeader></Card>
    </section>
    <Card v-else class="shrink-0"><CardContent v-if="management" class="flex flex-col gap-4">      <div v-if="management" class="flex min-w-0 items-center gap-2">
        <Select v-model="groupFilter"><SelectTrigger aria-label="筛选分组" class="w-32 shrink-0"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="all">全部分组</SelectItem><SelectItem value="ungrouped">未分组</SelectItem><SelectItem v-for="group in store.snapshot?.groups" :key="group.id" :value="group.id">{{ group.name }}</SelectItem></SelectGroup></SelectContent></Select>
        <InputGroup class="min-w-16 flex-1"><InputGroupInput id="reminder-search" v-model="query" placeholder="搜索提醒名称…" aria-label="搜索提醒" /><InputGroupAddon v-if="query.length" align="inline-end"><InputGroupButton size="icon-xs" aria-label="清空搜索" title="清空搜索" @click="clearSearch"><XIcon /></InputGroupButton></InputGroupAddon></InputGroup>
        <DropdownMenu><DropdownMenuTrigger as-child><Button variant="outline"><TagIcon data-icon="inline-start" />{{ tagFilter.length ? `标签 · ${tagFilter.length}` : '全部标签' }}<ChevronDownIcon data-icon="inline-end" /></Button></DropdownMenuTrigger><DropdownMenuContent align="start"><DropdownMenuGroup><DropdownMenuCheckboxItem :model-value="tagFilter.length === 0" @update:model-value="checked => { if (checked) tagFilter = [] }" @select.prevent>全部标签</DropdownMenuCheckboxItem><DropdownMenuCheckboxItem v-for="tag in store.snapshot?.tags" :key="tag.id" :model-value="tagFilter.includes(tag.id)" @update:model-value="checked => filterTag(tag.id, checked)" @select.prevent><span class="size-2 rounded-full" :style="{ backgroundColor: tag.color }" />{{ tag.name }}</DropdownMenuCheckboxItem></DropdownMenuGroup></DropdownMenuContent></DropdownMenu>
        <div class="ml-auto flex shrink-0 items-center gap-2"><Tabs v-model="displayMode"><TabsList aria-label="提醒显示方式"><TabsTrigger value="grid"><LayoutGridIcon />网格</TabsTrigger><TabsTrigger value="list"><ListIcon />列表</TabsTrigger></TabsList></Tabs>
          <ButtonGroup aria-label="提醒操作"><Button @click="createReminder()" :disabled="!store.snapshot || !!store.error || store.busy"><PlusIcon data-icon="inline-start" />新建提醒</Button><DropdownMenu><DropdownMenuTrigger as-child><Button size="icon" aria-label="更多提醒操作" :disabled="!store.snapshot || deletingBatch"><EllipsisIcon /></Button></DropdownMenuTrigger><DropdownMenuContent align="end"><DropdownMenuGroup><DropdownMenuItem :disabled="store.busy" @select="managing ? finishManaging() : managing = true">{{ managing ? '完成管理' : '批量管理' }}</DropdownMenuItem></DropdownMenuGroup></DropdownMenuContent></DropdownMenu></ButtonGroup>
        </div>
      </div>
      <div v-if="management && managing" class="flex flex-wrap items-center gap-3" role="group" aria-label="批量管理">
        <Checkbox id="select-visible" :model-value="selectedVisible.length > 0 && selectedVisible.length === filtered.length ? true : selectedVisible.length ? 'indeterminate' : false" :disabled="deletingBatch || store.busy || !filtered.length" @update:model-value="value => selectedIds = value === true ? filtered.map(a => a.id) : []" /><FieldLabel for="select-visible">全选当前结果</FieldLabel><span class="text-sm text-muted-foreground">已选 {{ selectedVisible.length }} 项</span>
        <Button size="sm" :disabled="!selectedVisible.length || store.busy || deletingBatch" @click="toggle(selectedVisible, true)">启用</Button><Button size="sm" variant="outline" :disabled="!selectedVisible.length || store.busy || deletingBatch" @click="toggle(selectedVisible, false)">停用</Button><Button size="sm" variant="destructive" :disabled="!selectedVisible.length || store.busy || deletingBatch" @click="batchDelete = true">删除</Button>
      </div>
</CardContent></Card>
    <Card class="reminder-content"><CardHeader v-if="!management" class="flex flex-row items-center justify-between gap-3"><CardTitle>我的提醒</CardTitle><Tabs v-model="displayMode"><TabsList aria-label="提醒显示方式"><TabsTrigger value="grid"><LayoutGridIcon />网格</TabsTrigger><TabsTrigger value="list"><ListIcon />列表</TabsTrigger></TabsList></Tabs></CardHeader><ScrollArea class="min-h-0 flex-1"><CardContent class="flex flex-col gap-6">
    <Alert v-if="store.error" variant="destructive"><AlertTitle>需要处理</AlertTitle><AlertDescription>{{ store.error }}<Button variant="outline" size="sm" class="mt-2" @click="chooseRoot">重新选择数据目录</Button></AlertDescription></Alert>
    <p v-if="store.loading" role="status" class="text-muted-foreground">正在读取本地数据…</p>
    <Empty v-else-if="!store.snapshot"><EmptyHeader><EmptyMedia><BrandLogo class="size-14" /></EmptyMedia><EmptyTitle>欢迎使用 onTime</EmptyTitle><EmptyDescription>选择数据目录，开始创建提醒。</EmptyDescription></EmptyHeader><EmptyContent><Button @click="chooseRoot" :disabled="store.busy">选择数据目录</Button></EmptyContent></Empty>
    <template v-else>
      <Card v-if="!management && recentMissed.length" size="sm"><CardHeader><CardTitle>过期未补响</CardTitle><CardDescription>应用未运行或电脑休眠超过补响时限的提醒，最近 90 天最多显示 3 条。</CardDescription></CardHeader><CardContent class="flex flex-col gap-3"><div v-for="occurrence in recentMissed" :key="occurrence.id" class="flex flex-wrap items-center justify-between gap-3 rounded-md border p-3"><div class="min-w-0"><p class="truncate font-medium">{{ occurrence.title }}</p><p class="text-sm text-muted-foreground">计划 {{ dateLabel(occurrence.scheduledAt) }} · {{ occurrence.reason }}</p></div><Button v-if="store.snapshot.alarms.some(alarm => alarm.id === occurrence.alarmId)" variant="outline" size="sm" @click="editMissed(occurrence.id)">调整提醒</Button></div></CardContent></Card>
      <div v-if="filtered.length" :class="cn('grid gap-4', displayMode === 'grid' ? 'reminder-grid' : 'reminder-list')"><Card v-for="a in filtered" :key="a.id" :size="displayMode === 'grid' ? 'sm' : 'default'" :data-selected="managing && selectedIds.includes(a.id)"><CardHeader><div class="flex items-start justify-between gap-3"><div><div class="flex items-center gap-2"><Checkbox v-if="management && managing" :model-value="selectedIds.includes(a.id)" :aria-label="`选择提醒 ${a.title}`" :disabled="deletingBatch || store.busy" @update:model-value="value => selectAlarm(a.id, value)" /><CardTitle class="font-semibold">{{ a.title }}</CardTitle></div><Badge v-if="displayMode === 'list'" class="mt-2" :variant="a.enabled ? 'default' : 'destructive'">{{ a.enabled ? '已启用' : '已停用' }}</Badge><CardDescription class="mt-2">{{ a.kind==='timer' ? '倒计时' : repeatLabels[a.repeat] }}</CardDescription></div><Switch v-if="displayMode === 'grid' || !management" :model-value="a.enabled" :aria-label="`${a.title}启用开关`" :disabled="store.busy" @update:model-value="v=>toggle([a.id],v)" /></div></CardHeader><CardContent><div v-if="displayMode === 'grid' || a.kind === 'timer'" class="flex items-center gap-3"><component :is="a.kind==='timer'?TimerIcon:AlarmClockIcon" class="size-5 text-muted-foreground" /><span class="text-2xl font-medium tabular-nums tracking-tight">{{ a.kind==='timer'?remaining(a):a.time }}</span></div><Badge v-if="displayMode === 'grid'" class="mt-2" :variant="a.enabled ? 'default' : 'destructive'">{{ a.enabled ? '已启用' : '已停用' }}</Badge><p v-if="a.kind==='clock' && a.enabled" class="mt-2 text-sm text-muted-foreground">下次：{{ remaining(a) }}</p><div v-if="a.groupId || a.tags.length" class="mt-3 flex flex-wrap gap-2"><Badge v-if="a.groupId" variant="secondary">{{ store.snapshot.groups.find(g => g.id === a.groupId)?.name }}</Badge><Badge v-for="tag in store.snapshot.tags.filter(t=>a.tags.includes(t.id))" :key="tag.id" variant="outline"><span class="size-2 rounded-full" :style="{backgroundColor:tag.color}" />{{ tag.name }}</Badge></div></CardContent><CardFooter v-if="management" class="flex justify-end gap-2 px-3 py-2"><div class="flex items-center gap-2"><Switch v-if="displayMode === 'list'" :model-value="a.enabled" :aria-label="`${a.title}启用开关`" :disabled="store.busy" @update:model-value="v => toggle([a.id], v)" /><Tooltip v-if="management"><TooltipTrigger as-child><Button variant="ghost" size="icon-sm" aria-label="编辑提醒" @click="router.push({name:'edit-reminder',params:{id:a.id},query:{from:'/reminders'}})"><PencilIcon /></Button></TooltipTrigger><TooltipContent>编辑提醒</TooltipContent></Tooltip><Tooltip v-if="management"><TooltipTrigger as-child><Button variant="destructive" size="icon-sm" aria-label="删除提醒" @click="deletion=a"><Trash2Icon /></Button></TooltipTrigger><TooltipContent>删除提醒</TooltipContent></Tooltip></div></CardFooter></Card></div>
      <Empty v-else><EmptyHeader><EmptyMedia variant="icon"><AlarmClockIcon /></EmptyMedia><EmptyTitle>{{ store.snapshot.alarms.length?'没有匹配的提醒':'从第一个提醒开始' }}</EmptyTitle><EmptyDescription>{{ management ? '点击下方按钮创建提醒。' : '在“全部提醒”中创建和管理提醒。' }}</EmptyDescription></EmptyHeader><EmptyContent v-if="management"><Button variant="outline" @click="createReminder()">新建提醒</Button></EmptyContent></Empty>
    </template>

    </CardContent></ScrollArea></Card>
    <Dialog v-model:open="batchDelete"><DialogContent @interact-outside="event => { if (deletingBatch) event.preventDefault() }" @escape-key-down="event => { if (deletingBatch) event.preventDefault() }"><DialogHeader><DialogTitle>删除选中的 {{ selectedVisible.length }} 个提醒？</DialogTitle><DialogDescription>将删除所选提醒并取消它们的活动提醒，历史记录保留。</DialogDescription></DialogHeader><DialogFooter><Button variant="outline" :disabled="deletingBatch" @click="batchDelete = false">取消</Button><Button variant="destructive" :disabled="deletingBatch || store.busy" @click="removeBatch">{{ deletingBatch ? '正在删除…' : '确认删除' }}</Button></DialogFooter></DialogContent></Dialog>
    <Dialog :open="!!deletion" @update:open="v=>{if(!v) deletion=null}"><DialogContent><DialogHeader><DialogTitle>删除提醒？</DialogTitle><DialogDescription>将删除“{{ deletion?.title }}”并取消它的活动提醒，历史记录保留。</DialogDescription></DialogHeader><DialogFooter><Button variant="outline" @click="deletion=null">取消</Button><Button variant="destructive" @click="remove" :disabled="store.busy">删除</Button></DialogFooter></DialogContent></Dialog>
    <Dialog :open="!!expiredAlarm" @update:open="v => { if (!v) expiredAlarm=null }"><DialogContent><DialogHeader><DialogTitle>请先调整提醒时间</DialogTitle><DialogDescription>“{{ expiredAlarm?.title }}”的一次性时间已过。请选择未来的日期和时间后保存启用。</DialogDescription></DialogHeader><DialogFooter><Button variant="outline" @click="expiredAlarm=null">取消</Button><Button @click="router.push({name:'edit-reminder',params:{id:expiredAlarm!.id}})">编辑提醒</Button></DialogFooter></DialogContent></Dialog>
  </main>
</template>
