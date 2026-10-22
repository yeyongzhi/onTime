<script setup lang="ts">
import { computed, ref } from 'vue'
import { useElementSize } from '@vueuse/core'
import { AlarmClockIcon, TimerIcon } from '@lucide/vue'
import type { Alarm } from '@/lib/types'
import { alarmEffective, dateLabel, repeatLabels } from '@/lib/types'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger, PopoverHeader, PopoverTitle, PopoverDescription } from '@/components/ui/popover'
import { ScrollArea } from '@/components/ui/scroll-area'
const props = defineProps<{ alarm: Alarm; nextRun?: number; now: number; compact?: boolean }>()
const times = computed(() => [...new Set(props.alarm.repeat === 'once' ? [props.alarm.time] : props.alarm.times?.length ? props.alarm.times : [props.alarm.time])].sort())
const next = computed(() => props.alarm.enabled && props.nextRun !== undefined && props.nextRun >= props.now ? props.nextRun : undefined)
const timeText = (date: Date) => `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`
const nextTime = computed(() => next.value === undefined ? undefined : timeText(new Date(next.value * 1000)))
const isNext = (time: string) => time === nextTime.value
function countdown(seconds: number) {
  seconds = Math.max(0, seconds)
  return `${Math.floor(seconds / 3600)}:${String(Math.floor(seconds % 3600 / 60)).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`
}
const primaryTime = computed(() => props.alarm.kind === 'timer'
  ? countdown(next.value === undefined ? props.alarm.durationSeconds : next.value - props.now)
  : nextTime.value ?? times.value[0] ?? props.alarm.time)
const dayLabel = computed(() => {
  if (!alarmEffective(props.alarm, props.now)) return '已失效'
  if (!props.alarm.enabled) return '已停用'
  if (next.value === undefined) return props.alarm.kind === 'timer' ? '未开始' : '暂无下次'
  const date = new Date(next.value * 1000), now = new Date(props.now * 1000)
  const tomorrow = new Date(now); tomorrow.setDate(tomorrow.getDate() + 1)
  if (date.toDateString() === now.toDateString()) return '今天'
  if (date.toDateString() === tomorrow.toDateString()) return '明天'
  return new Intl.DateTimeFormat('zh-CN', { ...(date.getFullYear() !== now.getFullYear() ? { year: 'numeric' as const } : {}), month: 'long', day: 'numeric' }).format(date)
})
const intervalLabel = computed(() => props.alarm.intervalMinutes === 30 ? '每隔半小时' : props.alarm.intervalMinutes === 60 ? '每隔 1 小时' : '每隔 2 小时')
const ruleLabel = computed(() => {
  if (props.alarm.kind === 'timer') return next.value === undefined ? '倒计时 · 尚未开始计时' : `截止：${dateLabel(next.value)}`
  if (props.alarm.repeat === 'once') return `单次提醒${props.alarm.date ? ` · ${props.alarm.date.replaceAll('-', '/')}` : ''}`
  return `${repeatLabels[props.alarm.repeat]} · ${props.alarm.intervalMinutes ? intervalLabel.value : `共 ${times.value.length} 个时间点`}`
})
const showTimes = computed(() => props.alarm.kind === 'clock' && props.alarm.repeat !== 'once' && !props.alarm.intervalMinutes)
const row = ref<HTMLElement>(), measurement = ref<HTMLElement>()
const { width } = useElementSize(row)
const { width: badgeWidth } = useElementSize(measurement)
const visibleCount = computed(() => {
  const unit = badgeWidth.value || 48, gap = 6
  const fit = Math.floor((width.value + gap) / (unit + gap))
  if (times.value.length <= Math.min(3, fit)) return times.value.length
  return Math.max(0, Math.min(3, Math.floor((width.value - unit - gap) / (unit + gap))))
})
const hiddenCount = computed(() => times.value.length - visibleCount.value)
</script>
<template>
  <div class="alarm-schedule-summary">
    <div class="flex min-w-0 flex-wrap items-center gap-2" :title="next !== undefined ? `下一次提醒：${dateLabel(next)}` : undefined">
      <component :is="alarm.kind === 'timer' ? TimerIcon : AlarmClockIcon" class="size-5 shrink-0 text-muted-foreground" aria-hidden="true" />
      <span class="text-lg font-medium tabular-nums tracking-tight whitespace-nowrap">{{ primaryTime }}</span>
      <Badge variant="outline" class="ml-auto shrink-0">{{ dayLabel }}</Badge>
    </div>
    <p class="min-w-0 text-sm text-muted-foreground whitespace-normal break-words">{{ ruleLabel }}</p>
    <div ref="row" class="reminder-time-tags relative flex min-w-0 items-center gap-1.5" :aria-label="showTimes ? '提醒时间摘要' : undefined">
      <template v-if="showTimes">
        <div ref="measurement" aria-hidden="true" class="invisible pointer-events-none absolute w-max"><Badge variant="secondary" class="tabular-nums">00:00</Badge></div>
        <Badge v-for="time in compact ? [] : times.slice(0, visibleCount)" :key="time" :variant="isNext(time) ? 'default' : 'secondary'" class="shrink-0 tabular-nums" :aria-label="isNext(time) ? `${time}，下一次提醒` : time">{{ time }}</Badge>
        <Popover v-if="compact ? times.length > 1 : hiddenCount > 0">
          <PopoverTrigger as-child><Button variant="secondary" size="xs" class="tabular-nums" :aria-label="compact ? `查看全部 ${times.length} 个提醒时间` : `查看全部 ${times.length} 个提醒时间，另有 ${hiddenCount} 个`">{{ compact ? `查看全部 ${times.length} 个时间` : `+${hiddenCount}` }}</Button></PopoverTrigger>
          <PopoverContent align="start" class="w-72" :aria-label="`${alarm.title}的全部提醒时间`">
            <PopoverHeader><PopoverTitle>提醒时间</PopoverTitle><PopoverDescription>{{ repeatLabels[alarm.repeat] }} · 共 {{ times.length }} 个时间点</PopoverDescription></PopoverHeader>
            <ScrollArea class="mt-3" :style="{ height: `${Math.min(Math.ceil(times.length / 3) * 3.25, 16)}rem` }">
              <div class="grid grid-cols-3 gap-2 pr-2">
                <div v-for="time in times" :key="time" class="flex min-w-0 flex-col items-center gap-1">
                  <Badge :variant="isNext(time) ? 'default' : 'secondary'" class="justify-center tabular-nums" :aria-label="isNext(time) ? `${time}，下一次提醒` : time">{{ time }}</Badge>
                  <span class="min-h-4 text-xs text-muted-foreground">{{ isNext(time) ? '下一次' : '' }}</span>
                </div>
              </div>
            </ScrollArea>
          </PopoverContent>
        </Popover>
      </template>
    </div>
  </div>
</template>
