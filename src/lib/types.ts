export interface Alarm {
  id: string; title: string; content: string; enabled: boolean; kind: 'clock' | 'timer'; time: string;
  date: string | null; repeat: 'once' | 'daily' | 'workdays' | 'weekends' | 'custom' | 'monthly' | 'quarterly' | 'yearly';
  times?: string[]; intervalMinutes?: number | null; scheduleStart?: number | null;
  monthlyDay: number | null;
  effectiveStart: string | null; effectiveEnd: string | null;
  weekdays: number[]; durationSeconds: number; deadline: number | null;
  sound: string | null; volume: number; gradual: boolean; ringSeconds: number; snoozeLimit: number;
  groupId: string | null; tags: string[]; revision: number; createdAt: number;
}
export interface Tag { createdAt?: number | null; id: string; name: string; color: string }
export interface Occurrence {
  context?: { inferred?: boolean; kind: string; group: Tag | null; tags: Tag[] } | null;
  id: string; alarmId: string; title: string; scheduledAt: number; status: string;
  snoozeCount: number; snoozeUntil: number | null; startedAt: number | null; reason: string;
}
export interface Calendar { year: number; source: string; fetchedAt: number; dates: Record<string, boolean> }
export interface Snapshot { dataRoot: string; alarms: Alarm[]; tags: Tag[]; groups: Tag[]; occurrences: Occurrence[]; calendars: Calendar[]; nextRuns: Record<string, number> }
export const repeatLabels: Record<Alarm['repeat'], string> = { once: '单次', daily: '每天', custom: '每周', monthly: '每月', quarterly: '每季度', yearly: '每年', workdays: '工作日', weekends: '周末' }
export const statusLabels: Record<string, string> = { ringing: '响铃中', snoozed: '贪睡中', dismissed: '已停止', auto_stopped: '自动停止', skipped_expired: '过期未补响', cancelled: '已取消' }
export function dateLabel(seconds: number) { return new Date(seconds * 1000).toLocaleString('zh-CN', { year: 'numeric', month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' }) }
export function newAlarm(): Alarm {
  const date = new Date(); date.setMinutes(date.getMinutes() + 5)
  const pad = (n: number) => String(n).padStart(2, '0')
  return { id: '', title: '', content: '', enabled: true, kind: 'clock', time: `${pad(date.getHours())}:${pad(date.getMinutes())}`, date: `${date.getFullYear()}-${pad(date.getMonth()+1)}-${pad(date.getDate())}`, repeat: 'daily', monthlyDay: null, effectiveStart: null, effectiveEnd: null, weekdays: [], durationSeconds: 1500, deadline: null, sound: null, volume: 0.7, gradual: true, ringSeconds: 60, snoozeLimit: 3, groupId: null, tags: [], revision: 0, createdAt: 0 }
}

// Effectiveness is independent of the user's enabled switch and nextRuns.
export function alarmEffective(a: Alarm, now: number): boolean {
  if (a.kind === 'timer') return a.deadline === null || a.deadline > now
  if (a.repeat === 'once') return !!a.date && new Date(`${a.date}T${a.time}:00`).getTime() > now * 1000
  return !a.effectiveEnd || new Date(`${a.effectiveEnd}T23:59:59`).getTime() >= now * 1000
}
