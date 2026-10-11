export interface Alarm {
  id: string; title: string; enabled: boolean; kind: 'clock' | 'timer'; time: string;
  date: string | null; repeat: 'once' | 'daily' | 'workdays' | 'weekends' | 'custom';
  weekdays: number[]; skipHolidays: boolean; durationSeconds: number; deadline: number | null;
  sound: string | null; volume: number; gradual: boolean; ringSeconds: number; snoozeLimit: number;
  tags: string[]; revision: number; createdAt: number;
}
export interface Tag { id: string; name: string; color: string }
export interface Occurrence {
  id: string; alarmId: string; title: string; scheduledAt: number; status: string;
  snoozeCount: number; snoozeUntil: number | null; startedAt: number | null; reason: string;
}
export interface Calendar { year: number; source: string; fetchedAt: number; dates: Record<string, boolean> }
export interface Snapshot { dataRoot: string; alarms: Alarm[]; tags: Tag[]; occurrences: Occurrence[]; calendars: Calendar[]; nextRuns: Record<string, number> }
export const repeatLabels = { once: '一次性', daily: '每天', workdays: '工作日', weekends: '周末', custom: '自定义星期' }
export const statusLabels: Record<string, string> = { ringing: '响铃中', snoozed: '贪睡中', dismissed: '已停止', auto_stopped: '自动停止', skipped_expired: '过期未补响', cancelled: '已取消' }
export function dateLabel(seconds: number) { return new Date(seconds * 1000).toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' }) }
export function newAlarm(): Alarm {
  const date = new Date(); date.setMinutes(date.getMinutes() + 5)
  const pad = (n: number) => String(n).padStart(2, '0')
  return { id: '', title: '', enabled: true, kind: 'clock', time: `${pad(date.getHours())}:${pad(date.getMinutes())}`, date: `${date.getFullYear()}-${pad(date.getMonth()+1)}-${pad(date.getDate())}`, repeat: 'daily', weekdays: [0,1,2,3,4], skipHolidays: false, durationSeconds: 1500, deadline: null, sound: null, volume: 0.7, gradual: true, ringSeconds: 60, snoozeLimit: 3, tags: [], revision: 0, createdAt: 0 }
}