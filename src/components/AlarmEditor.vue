<script setup lang="ts">
import { onBeforeRouteLeave, onBeforeRouteUpdate } from 'vue-router'
import { computed, reactive, ref, onUnmounted, watch, nextTick } from 'vue'
import { Music2Icon, PlayIcon, SquareIcon, ArrowLeftIcon, CalendarDaysIcon } from '@lucide/vue'
import { getLocalTimeZone, parseDate, today, type DateValue } from '@internationalized/date'
import type { Alarm } from '@/lib/types'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { useAlarmsStore } from '@/stores/alarms'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { Switch } from '@/components/ui/switch'
import { FieldGroup, Field, FieldLabel, FieldDescription, FieldSet, FieldLegend, FieldError } from '@/components/ui/field'
import { Select, SelectTrigger, SelectValue, SelectContent, SelectGroup, SelectItem } from '@/components/ui/select'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { Card, CardContent } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Slider } from '@/components/ui/slider'
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert'
import { Checkbox } from '@/components/ui/checkbox'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Calendar } from '@/components/ui/calendar'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
const props = defineProps<{ alarm: Alarm }>()
const emit = defineEmits<{ close: [] }>()
const store = useAlarmsStore()
const form = reactive<Alarm>(JSON.parse(JSON.stringify(props.alarm)))
if (form.repeat === 'workdays' || form.repeat === 'weekends') {
  form.weekdays = form.repeat === 'workdays' ? [0, 1, 2, 3, 4] : [5, 6]
  form.repeat = 'custom'
}
const group = computed({ get: () => form.groupId ?? 'ungrouped', set: (value: string) => { form.groupId = value === 'ungrouped' ? null : value } })
const minutes = ref(form.durationSeconds / 60)
const selectedDays = ref(form.weekdays.map(String))
const initialMonthlyDay = form.monthlyDay ?? (form.repeat === 'monthly' && form.date ? Number(form.date.slice(-2)) : 1)
const monthlyDayChoice = ref(initialMonthlyDay === 0 ? 'end' : ['1', '10', '15'].includes(String(initialMonthlyDay)) ? String(initialMonthlyDay) : 'custom')
const customMonthlyDay = ref(String(initialMonthlyDay))
const effectivePeriodMode = ref(form.effectiveStart || form.effectiveEnd ? 'custom' : 'long-term')
const reminderMode = computed({
  get: () => form.kind === 'timer' ? 'timer' : form.repeat === 'once' ? 'single' : 'repeat',
  set: (value: string) => {
    if (value === 'timer') form.kind = 'timer'
    else {
      form.kind = 'clock'
      if (value === 'single') {
        form.repeat = 'once'
        form.effectiveStart = null
        form.effectiveEnd = null
        effectivePeriodMode.value = 'long-term'
      }
      else if (form.repeat === 'once') form.repeat = 'daily'
    }
  },
})
const datePopoverOpen = ref(false)
const selectedDate = computed<DateValue | undefined>({
  get: () => form.date ? parseDate(form.date) : undefined,
  set: value => { form.date = value?.toString() ?? null },
})
const effectiveStartPopoverOpen = ref(false)
const effectiveEndPopoverOpen = ref(false)
const effectiveStartDate = computed<DateValue | undefined>({
  get: () => form.effectiveStart ? parseDate(form.effectiveStart) : undefined,
  set: value => { form.effectiveStart = value?.toString() ?? null },
})
const effectiveEndDate = computed<DateValue | undefined>({
  get: () => form.effectiveEnd ? parseDate(form.effectiveEnd) : undefined,
  set: value => { form.effectiveEnd = value?.toString() ?? null },
})
const formatDate = (value?: DateValue) => value ? new Intl.DateTimeFormat('zh-CN', { dateStyle: 'long' }).format(value.toDate(getLocalTimeZone())) : '选择日期'
const dateText = computed(() => selectedDate.value
  ? formatDate(selectedDate.value)
  : '选择日期')
const timeHour = computed({ get: () => (form.time || '09:00').slice(0, 2), set: (value: string) => { form.time = `${value}:${(form.time || '09:00').slice(3, 5)}` } })
const timeMinute = computed({ get: () => (form.time || '09:00').slice(3, 5), set: (value: string) => { form.time = `${(form.time || '09:00').slice(0, 2)}:${value}` } })
const hourOptions = Array.from({ length: 24 }, (_, i) => String(i).padStart(2, '0'))
const minuteOptions = Array.from({ length: 60 }, (_, i) => String(i).padStart(2, '0'))
const volume = computed({ get: () => [Math.round(form.volume * 100)], set: (value: number[]) => { if(value[0] !== undefined) form.volume = value[0] / 100 } })
const editorTab = ref('schedule')
const previewing = ref(false)
let previewTimer: ReturnType<typeof setTimeout> | undefined
onUnmounted(() => { if(previewTimer)clearTimeout(previewTimer); if(previewing.value)void invoke('preview_sound',{sound:null,volume:0,stop:true}).catch(()=>{}) })
const formError = ref('')
const errorTarget = ref('')
function invalid(message: string, target: string, tab = 'schedule') { errorTarget.value = target; editorTab.value = tab; formError.value = message }
const saved = ref(false)
const restartTimer = ref(false)
const leaveOpen = ref(false)
let resolveLeave: ((value: boolean) => void) | undefined
const editorState = () => JSON.stringify([form, minutes.value, selectedDays.value, monthlyDayChoice.value, customMonthlyDay.value, effectivePeriodMode.value, restartTimer.value])
const initialEditorState = editorState()
const dirty = computed(() => !saved.value && editorState() !== initialEditorState)
function confirmLeave() { if (!dirty.value) return true; leaveOpen.value = true; return new Promise<boolean>(resolve => { resolveLeave = resolve }) }
function answerLeave(leave: boolean) { leaveOpen.value = false; resolveLeave?.(leave); resolveLeave = undefined }
onBeforeRouteLeave(confirmLeave)
onBeforeRouteUpdate(confirmLeave)
onUnmounted(() => { resolveLeave?.(false) })
watch(formError, async value => {
  if (!value) return
  if (/音量|响铃|贪睡|自动停止/.test(value)) editorTab.value = 'sound'
  await nextTick()
  const target = document.getElementById(errorTarget.value) ?? document.getElementById('alarm-form-error')
  target?.focus(); target?.scrollIntoView({ block: 'nearest' })
})
const conflictOpen = ref(false)
const conflictNames = ref<string[]>([])
const nextPreview = ref<number[] | undefined>(undefined)
const previewingSchedule = ref(false)
let schedulePreviewTimer: ReturnType<typeof setTimeout> | undefined
const nextPreviewText = computed(() => {
  if (previewingSchedule.value) return '正在计算下一次提醒…'
  if (form.kind === 'clock' && form.repeat === 'custom' && !selectedDays.value.length) return '请选择至少一个重复星期'
  if (nextPreview.value === undefined) return '下一次提醒暂时无法预览。'
  if (!nextPreview.value.length) return '当前规则没有下一次提醒，请检查生效日期和提醒日期。'
  const formatted = nextPreview.value.map(value => new Date(value * 1000).toLocaleString('zh-CN', { dateStyle: 'medium', timeStyle: 'short' }))
  return formatted.length === 1 ? `下一次提醒：${formatted[0]}` : `接下来 ${formatted.length} 次：${formatted.join('、')}`
})
let previewSequence = 0
async function refreshNextPreview() {
  const sequence = ++previewSequence
  if (schedulePreviewTimer) clearTimeout(schedulePreviewTimer)
  previewingSchedule.value = true
  schedulePreviewTimer = setTimeout(async () => {
    const alarm = JSON.parse(JSON.stringify(form)) as Alarm
    if (alarm.kind === 'timer' && (restartTimer.value || !props.alarm.enabled || Number(minutes.value) * 60 !== props.alarm.durationSeconds)) alarm.deadline = null
    alarm.enabled = true
    alarm.weekdays = selectedDays.value.map(Number)
    if (alarm.kind === 'timer') alarm.durationSeconds = Math.round(Number(minutes.value) * 60)
    if (alarm.kind === 'clock' && alarm.repeat === 'monthly') {
      alarm.monthlyDay = monthlyDayChoice.value === 'end' ? 0 : Number(monthlyDayChoice.value === 'custom' ? customMonthlyDay.value : monthlyDayChoice.value)
    }
    try {
      const result = await invoke<number[]>('preview_next_runs', { alarm, count: alarm.kind === 'timer' || alarm.repeat === 'once' ? 1 : 3 })
      if (sequence === previewSequence) nextPreview.value = result
    } catch {
      if (sequence === previewSequence) nextPreview.value = undefined
    } finally {
      if (sequence === previewSequence) {
        schedulePreviewTimer = undefined
        previewingSchedule.value = false
      }
    }
  }, 180)
}
watch(() => [form.kind, form.repeat, form.time, form.date, form.effectiveStart, form.effectiveEnd, monthlyDayChoice.value, customMonthlyDay.value, selectedDays.value.join(','), minutes.value, restartTimer.value], refreshNextPreview, { immediate: true })
onUnmounted(() => { if (schedulePreviewTimer) clearTimeout(schedulePreviewTimer) })
async function importSound() { try { const sound = await store.action<string | null>('import_sound'); if(sound) { form.sound = sound; toast.success('铃声已导入') } } catch { /* toast reports */ } }
async function preview() { try { await store.action('preview_sound', {sound:form.sound,volume:Number(form.volume),stop:previewing.value}); previewing.value=!previewing.value; if(previewTimer)clearTimeout(previewTimer); if(previewing.value)previewTimer=setTimeout(()=>{previewing.value=false},10000) } catch { /* toast reports */ } }
function useBuiltInSound() { form.sound = null; store.audioError = ''; toast.success('已切换为内置轻铃') }
async function close() { if(store.busy)return; if(previewing.value) { try { await store.action('preview_sound',{sound:null,volume:0,stop:true}) } catch { /* toast */ } } emit('close') }
async function save() {
  formError.value=''; errorTarget.value=''
  if(!form.title.trim()) { invalid('请填写提醒标题', 'alarm-title'); return }
  if(form.kind === 'timer' && (!Number.isFinite(Number(minutes.value)) || Number(minutes.value) * 60 < 1 || Number(minutes.value) * 60 > 604800)) { invalid('倒计时需为 1 秒到 7 天', 'minutes'); return }
  if (!Number.isInteger(Number(form.ringSeconds)) || Number(form.ringSeconds) < 1 || Number(form.ringSeconds) > 3600) { invalid('自动停止需为 1–3600 秒', 'alarm-duration', 'sound'); return }
  if (!Number.isInteger(Number(form.snoozeLimit)) || Number(form.snoozeLimit) < 0 || Number(form.snoozeLimit) > 10) { invalid('贪睡次数需为 0–10', 'alarm-snooze', 'sound'); return }
  if(form.kind==='clock' && form.repeat==='custom' && !selectedDays.value.length) { invalid('请选择重复星期', 'alarm-weekdays'); return }
  if(form.kind==='clock' && ['once', 'yearly'].includes(form.repeat) && !form.date) { invalid('请选择提醒日期', 'alarm-date-trigger'); return }
  if(form.kind==='clock' && form.repeat!=='once' && effectivePeriodMode.value==='custom') {
    if (!form.effectiveStart || !form.effectiveEnd) { invalid('请选择完整的提醒生效日期范围', 'effective-period'); return }
    if (form.effectiveStart > form.effectiveEnd) { invalid('生效开始日期不能晚于结束日期', 'effective-period'); return }
  }
  if (effectivePeriodMode.value === 'long-term') { form.effectiveStart = null; form.effectiveEnd = null }
  if(form.kind==='clock' && form.repeat==='monthly') form.monthlyDay = monthlyDayChoice.value === 'end' ? 0 : Number(monthlyDayChoice.value === 'custom' ? customMonthlyDay.value : monthlyDayChoice.value)
  form.durationSeconds=Math.round(Number(minutes.value)*60); form.weekdays=selectedDays.value.map(Number)
  form.volume=Number(form.volume); form.ringSeconds=Number(form.ringSeconds); form.snoozeLimit=Number(form.snoozeLimit)
  if (form.enabled) {
    try {
      conflictNames.value = await invoke<string[]>('check_alarm_conflicts', { alarm: JSON.parse(JSON.stringify(form)) })
      if (conflictNames.value.length) { conflictOpen.value = true; return }
    } catch (e) { formError.value = `检查时间冲突失败：${String(e)}`; return }
  }
  await persist()
}
async function persist() {
  conflictOpen.value = false
  try { await store.action('save_alarm',{alarm:form,restartTimer:restartTimer.value},'提醒已保存'); saved.value=true; await close() } catch(e) { formError.value=String(e) }
}
function toggleWeekday(day: number, checked: boolean | 'indeterminate') {
  const value = String(day)
  selectedDays.value = checked === true
    ? [...new Set([...selectedDays.value, value])].sort((a, b) => Number(a) - Number(b))
    : selectedDays.value.filter(item => item !== value)
}
function setRepeat(value: unknown) {
  if (typeof value === 'string' && ['daily', 'custom', 'monthly', 'quarterly', 'yearly'].includes(value)) {
    if (value === 'custom' && form.repeat !== 'custom') selectedDays.value = []
    form.repeat = value as Alarm['repeat']
    if (value === 'monthly') {
      form.date = null
      form.monthlyDay ??= initialMonthlyDay
    }
    if (value === 'quarterly') form.date = null
    if (value === 'yearly' && !form.date) form.date = today(getLocalTimeZone()).toString()
  }
}
function setEffectivePeriod(value: unknown) {
  if (value !== 'long-term' && value !== 'custom') return
  effectivePeriodMode.value = value
  if (value === 'custom') form.effectiveStart ||= today(getLocalTimeZone()).toString()
  else { form.effectiveStart = null; form.effectiveEnd = null }
}
</script>
<template>
  <main class="editor-page">
    <header class="editor-heading">
      <div class="flex items-center justify-between gap-3"><h1 class="workspace-heading">{{ alarm.id ? '编辑提醒' : '新建提醒' }}</h1><Button variant="secondary" :disabled="store.busy" @click="close"><ArrowLeftIcon />返回</Button></div><p class="mt-1 text-sm text-muted-foreground">先安排时间与重复方式，再按需添加内容和调整铃声。</p>
    </header><Tabs v-model="editorTab" class="editor-tabs"><TabsList class="w-full" aria-label="提醒设置分类"><TabsTrigger value="schedule" class="flex-1">提醒安排</TabsTrigger><TabsTrigger value="sound" class="flex-1">铃声与高级</TabsTrigger></TabsList><div class="editor-surface"><ScrollArea class="min-h-0 flex-1"><CardContent class="editor-scroll-content flex flex-col gap-5">
    <Alert id="alarm-form-error" tabindex="-1" role="alert" v-if="formError" variant="destructive"><AlertTitle>无法保存提醒</AlertTitle><AlertDescription>{{ formError }}</AlertDescription></Alert>
    <form novalidate id="alarm-form" @submit.prevent="save" class="flex flex-col gap-5">
      <TabsContent value="schedule"><FieldSet :disabled="store.busy"><FieldGroup class="alarm-schedule-fields">
        <Card class="alarm-wide alarm-section-card" size="sm"><CardContent class="alarm-section-fields"><Field :data-invalid="!!formError && /标题/.test(formError)"><FieldLabel for="alarm-title">标题</FieldLabel><Input id="alarm-title" v-model="form.title" placeholder="例如：午休结束、吃药、团队会议" maxlength="80" :aria-invalid="!!formError && /标题/.test(formError)" aria-describedby="alarm-form-error" required autofocus /><FieldError v-if="errorTarget === 'alarm-title'">{{ formError }}</FieldError></Field><Field><FieldLabel for="alarm-content">内容</FieldLabel><Textarea id="alarm-content" v-model="form.content" placeholder="添加备注、地点或需要准备的事项…" maxlength="2000" rows="2" /></Field></CardContent></Card>
        <Card v-if="reminderMode === 'repeat'" class="alarm-wide alarm-section-card" size="sm">
          <CardContent class="alarm-effective-date-content">
            <Field>
              <FieldLabel for="effective-period">提醒生效日期</FieldLabel>
              <Select :model-value="effectivePeriodMode" @update:model-value="setEffectivePeriod">
                <SelectTrigger id="effective-period" class="w-full"><SelectValue /></SelectTrigger>
                <SelectContent><SelectGroup><SelectItem value="long-term">长期有效</SelectItem><SelectItem value="custom">自定义日期范围</SelectItem></SelectGroup></SelectContent>
              </Select>
              <div v-if="effectivePeriodMode === 'custom'" class="alarm-date-range">
                <div class="alarm-date-range-item"><span class="text-sm text-muted-foreground">开始日期</span><Popover v-model:open="effectiveStartPopoverOpen"><PopoverTrigger as-child><Button type="button" variant="outline" class="alarm-date-trigger justify-start text-left font-normal"><CalendarDaysIcon class="mr-2 size-4" />{{ formatDate(effectiveStartDate) }}</Button></PopoverTrigger><PopoverContent class="w-auto p-0" align="start"><Calendar v-model="effectiveStartDate" :default-placeholder="today(getLocalTimeZone())" :max-value="effectiveEndDate" locale="zh-CN" layout="month-and-year" :week-starts-on="1" @update:model-value="effectiveStartPopoverOpen = false" /></PopoverContent></Popover></div>
                <div class="alarm-date-range-item"><span class="text-sm text-muted-foreground">结束日期</span><Popover v-model:open="effectiveEndPopoverOpen"><PopoverTrigger as-child><Button type="button" variant="outline" class="alarm-date-trigger justify-start text-left font-normal"><CalendarDaysIcon class="mr-2 size-4" />{{ formatDate(effectiveEndDate) }}</Button></PopoverTrigger><PopoverContent class="w-auto p-0" align="start"><Calendar v-model="effectiveEndDate" :default-placeholder="effectiveStartDate ?? today(getLocalTimeZone())" :min-value="effectiveStartDate" locale="zh-CN" layout="month-and-year" :week-starts-on="1" @update:model-value="effectiveEndPopoverOpen = false" /></PopoverContent></Popover></div>
              </div>
            </Field>
          </CardContent>
        </Card>

        <Card class="alarm-wide alarm-section-card alarm-schedule-card" size="sm">
          <CardContent class="alarm-schedule-card-content">
            <Field>
              <FieldLabel id="reminder-kind-label">提醒方式</FieldLabel>
              <Tabs v-model="reminderMode" class="alarm-kind-tabs"><TabsList aria-labelledby="reminder-kind-label"><TabsTrigger value="single">单次提醒</TabsTrigger><TabsTrigger value="repeat">重复提醒</TabsTrigger><TabsTrigger value="timer">倒计时</TabsTrigger></TabsList></Tabs>
            </Field>
            <Field v-if="form.kind === 'timer'"><FieldLabel for="minutes">多少分钟后提醒</FieldLabel><Input :aria-invalid="errorTarget === 'minutes'" aria-describedby="alarm-form-error" id="minutes" v-model="minutes" type="number" min="0.02" max="10080" step="0.01" required /><FieldError v-if="errorTarget === 'minutes'">{{ formError }}</FieldError><FieldDescription>新建、修改时长或重新启用时开始计时，普通编辑保留截止时间；包含睡眠时间。</FieldDescription></Field>
            <template v-else>
              <Field v-if="reminderMode === 'single' || (reminderMode === 'repeat' && form.repeat === 'yearly')"><FieldLabel for="alarm-date-trigger">{{ reminderMode === 'single' ? '提醒日期' : '重复日期' }}</FieldLabel><Popover v-model:open="datePopoverOpen"><PopoverTrigger as-child><Button id="alarm-date-trigger" :aria-invalid="errorTarget === 'alarm-date-trigger'" aria-describedby="alarm-form-error" type="button" variant="outline" class="alarm-date-trigger justify-start text-left font-normal"><CalendarDaysIcon class="mr-2 size-4" />{{ dateText }}</Button></PopoverTrigger><PopoverContent class="w-auto p-0" align="start"><Calendar v-model="selectedDate" :default-placeholder="today(getLocalTimeZone())" locale="zh-CN" layout="month-and-year" :week-starts-on="1" @update:model-value="datePopoverOpen = false" /></PopoverContent></Popover></Field>
              <Field v-if="reminderMode === 'repeat' && form.repeat === 'monthly'"><FieldLabel for="monthly-day">提醒日期</FieldLabel><div class="alarm-monthly-options"><Select v-model="monthlyDayChoice"><SelectTrigger id="monthly-day" class="alarm-short-select"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="1">每月 1 号</SelectItem><SelectItem value="10">每月 10 号</SelectItem><SelectItem value="15">每月 15 号</SelectItem><SelectItem value="end">月底</SelectItem><SelectItem value="custom">自定义</SelectItem></SelectGroup></SelectContent></Select><Select v-if="monthlyDayChoice === 'custom'" v-model="customMonthlyDay"><SelectTrigger aria-label="选择每月几号" class="alarm-short-select"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem v-for="day in 31" :key="day" :value="String(day)">{{ day }} 号{{ day === 31 ? '（仅有31号的月份）' : '' }}</SelectItem></SelectGroup></SelectContent></Select></div></Field>
              <FieldSet v-if="reminderMode === 'repeat' && form.repeat === 'custom'"><FieldLegend>重复星期</FieldLegend><div id="alarm-weekdays" tabindex="-1" aria-describedby="alarm-form-error" class="alarm-weekdays"><label v-for="(day,i) in ['一','二','三','四','五','六','日']" :key="i" class="alarm-weekday-option"><Checkbox :model-value="selectedDays.includes(String(i))" @update:model-value="toggleWeekday(i, $event)" /><span>周{{ day }}</span></label></div></FieldSet>
              <Field v-if="reminderMode === 'repeat' && form.repeat === 'quarterly'"><FieldDescription>每年 3、6、9、12 月的最后一天提醒。</FieldDescription></Field>
              <div class="alarm-schedule-bottom">
                <Field>
                  <FieldLabel>提醒时间</FieldLabel>
                  <div class="alarm-time-selects"><Select v-model="timeHour"><SelectTrigger aria-label="小时"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem v-for="hour in hourOptions" :key="hour" :value="hour">{{ hour }}</SelectItem></SelectGroup></SelectContent></Select><span aria-hidden="true">:</span><Select v-model="timeMinute"><SelectTrigger aria-label="分钟"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem v-for="minute in minuteOptions" :key="minute" :value="minute">{{ minute }}</SelectItem></SelectGroup></SelectContent></Select></div>
                </Field>
                <Field v-if="reminderMode === 'repeat'" class="alarm-repeat-field"><FieldLabel for="alarm-repeat">重复维度</FieldLabel><Select :model-value="form.repeat" @update:model-value="setRepeat"><SelectTrigger id="alarm-repeat" class="w-full"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="daily">每天</SelectItem><SelectItem value="custom">每周</SelectItem><SelectItem value="monthly">每月</SelectItem><SelectItem value="quarterly">每季度</SelectItem><SelectItem value="yearly">每年</SelectItem></SelectGroup></SelectContent></Select></Field>
              </div>
            </template>
            <FieldDescription class="alarm-wide" role="status" aria-live="polite">{{ nextPreviewText }}</FieldDescription>
          </CardContent>
        </Card>

        <Card v-if="store.snapshot?.groups.length || store.snapshot?.tags.length" class="alarm-wide alarm-section-card" size="sm"><CardContent class="alarm-section-fields"><Field v-if="store.snapshot?.groups.length"><FieldLabel for="alarm-group">分组</FieldLabel><Select v-model="group"><SelectTrigger id="alarm-group" class="w-full"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="ungrouped">未分组</SelectItem><SelectItem v-for="item in store.snapshot?.groups" :key="item.id" :value="item.id">{{ item.name }}</SelectItem></SelectGroup></SelectContent></Select></Field><Field v-if="store.snapshot?.tags.length"><FieldLabel id="reminder-tags-label">标签</FieldLabel><ToggleGroup v-model="form.tags" type="multiple" variant="outline" :spacing="2" class="flex-wrap" aria-labelledby="reminder-tags-label"><ToggleGroupItem v-for="tag in store.snapshot.tags" :key="tag.id" :value="tag.id">{{ tag.name }}</ToggleGroupItem></ToggleGroup></Field></CardContent></Card>
      </FieldGroup></FieldSet></TabsContent>
      <TabsContent value="sound"><Field v-if="form.kind === 'timer' && alarm.id && alarm.enabled" orientation="horizontal"><FieldLabel for="restart-timer">保存时重新计时</FieldLabel><Switch id="restart-timer" v-model="restartTimer" /><FieldDescription>默认保留当前截止时间；修改倒计时时长也会重新计时。</FieldDescription></Field><Alert v-if="store.audioError" variant="destructive"><AlertTitle>最近一次铃声播放遇到问题</AlertTitle><AlertDescription class="flex flex-wrap items-center justify-between gap-3"><span>{{ store.audioError }}。若当前提醒使用自定义铃声，可切换为内置轻铃；也可试听确认当前音频输出。</span><Button type="button" variant="secondary" size="sm" :disabled="store.busy" @click="useBuiltInSound">改用内置轻铃</Button></AlertDescription></Alert><FieldSet :disabled="store.busy"><FieldGroup class="alarm-sound-fields">
        <Field class="alarm-wide"><FieldLabel>铃声</FieldLabel><div class="flex flex-wrap items-center gap-2"><Button type="button" variant="secondary" :disabled="store.busy" @click="importSound"><Music2Icon data-icon="inline-start" />{{ form.sound ? '更换音频' : '导入音频' }}</Button><Button type="button" variant="secondary" :disabled="store.busy" @click="form.sound=null">内置轻铃</Button><Button type="button" variant="secondary" :disabled="store.busy" @click="preview"><component :is="previewing ? SquareIcon : PlayIcon" data-icon="inline-start" />{{ previewing ? '停止试听' : '试听' }}</Button></div><FieldDescription>{{ form.sound ? '已选择本地音频' : '使用内置轻铃' }}</FieldDescription></Field>
        <Field class="alarm-wide"><FieldLabel id="volume-label">音量 {{ volume[0] }}%</FieldLabel><Slider id="alarm-volume" v-model="volume" :min="0" :max="100" :step="5" :disabled="store.busy" aria-labelledby="volume-label" /></Field>
        <Field class="alarm-wide" orientation="horizontal"><FieldLabel for="alarm-gradual">音量渐强</FieldLabel><Switch id="alarm-gradual" v-model="form.gradual" /></Field>
        <Field><FieldLabel for="alarm-duration">自动停止（秒）</FieldLabel><Input :aria-invalid="errorTarget === 'alarm-duration'" aria-describedby="alarm-form-error" id="alarm-duration" v-model="form.ringSeconds" type="number" min="1" max="3600" required /><FieldError v-if="errorTarget === 'alarm-duration'">{{ formError }}</FieldError></Field>
        <Field><FieldLabel for="alarm-snooze">最多延后次数</FieldLabel><Input :aria-invalid="errorTarget === 'alarm-snooze'" aria-describedby="alarm-form-error" id="alarm-snooze" v-model="form.snoozeLimit" type="number" min="0" max="10" required /><FieldError v-if="errorTarget === 'alarm-snooze'">{{ formError }}</FieldError><FieldDescription>响铃时可选 5 或 10 分钟后再响；两种选项共用此次数上限。设为 0 则不能延后。</FieldDescription></Field>
      </FieldGroup></FieldSet></TabsContent>
    </form>
    </CardContent></ScrollArea>
    <Card class="editor-footer-card" size="sm"><CardContent class="editor-footer"><div class="mr-auto flex min-w-0 items-center gap-2"><Switch id="alarm-enabled" v-model="form.enabled" :disabled="store.busy" /><FieldLabel for="alarm-enabled">保存后启用</FieldLabel></div><Button variant="secondary" :disabled="store.busy" @click="close">取消</Button><Button form="alarm-form" type="submit" :disabled="store.busy">{{ store.busy ? '正在保存…' : '保存提醒' }}</Button></CardContent></Card>
    </div></Tabs>
    <Dialog :open="leaveOpen" @update:open="value => { if (!value) answerLeave(false) }"><DialogContent><DialogHeader><DialogTitle>放弃未保存的修改？</DialogTitle><DialogDescription>离开后，本次编辑的内容将不会保存。</DialogDescription></DialogHeader><DialogFooter><Button variant="secondary" @click="answerLeave(false)">继续编辑</Button><Button variant="destructive" @click="answerLeave(true)">放弃并离开</Button></DialogFooter></DialogContent></Dialog>
    <Dialog v-model:open="conflictOpen"><DialogContent><DialogHeader><DialogTitle>发现时间接近的提醒</DialogTitle><DialogDescription>以下提醒将在 1 分钟内同时响铃：{{ conflictNames.join('、') }}。你仍可以保存；到时会合并系统通知，并在响铃窗口逐项处理。</DialogDescription></DialogHeader><DialogFooter><Button variant="secondary" @click="conflictOpen = false">返回调整</Button><Button @click="persist">仍然保存</Button></DialogFooter></DialogContent></Dialog>
  </main>
</template>



