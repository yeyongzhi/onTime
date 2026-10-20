import { alarmEffective, type Alarm, type Occurrence } from './types'

/** 提醒生效状态：统一维护文案和展示语义。 */
export const effectivenessDictionary = {
  effective: { label: '生效中', variant: 'success' },
  expired: { label: '已失效', variant: 'destructive' },
} as const

export type EffectivenessStatus = keyof typeof effectivenessDictionary

export function getEffectivenessStatus(alarm: Alarm, now: number): EffectivenessStatus {
  return alarmEffective(alarm, now) ? 'effective' : 'expired'
}

export function getEffectivenessDisplay(alarm: Alarm, now: number) {
  return effectivenessDictionary[getEffectivenessStatus(alarm, now)]
}

export const expirationReasonDictionary = {
  timer: '倒计时已结束',
  completed: '单次提醒已触发，提醒时间已过',
  missed: '单次提醒已过期，超过补响时限未补响',
  once: '单次提醒的设定时间已过',
  endDate: '已超过设置的生效结束日期',
} as const

export function getExpirationReason(alarm: Alarm, now: number, occurrences: Occurrence[] = []): string | null {
  if (alarmEffective(alarm, now)) return null
  if (alarm.kind === 'timer') return expirationReasonDictionary.timer
  if (alarm.repeat !== 'once') return expirationReasonDictionary.endDate
  const scheduledAt = new Date(`${alarm.date}T${alarm.time}:00`).getTime() / 1000
  const occurrence = occurrences.find(o => o.alarmId === alarm.id && o.scheduledAt === scheduledAt)
  if (occurrence?.status === 'skipped_expired') return expirationReasonDictionary.missed
  if (occurrence && ['ringing', 'snoozed', 'dismissed', 'auto_stopped'].includes(occurrence.status)) return expirationReasonDictionary.completed
  return expirationReasonDictionary.once
}
