export function nextIntervalStart(intervalMinutes: number, skipNext = false, now = new Date()): Date {
  const start = new Date(now)
  const boundary = intervalMinutes === 30 ? 30 : 60
  start.setSeconds(0, 0)
  start.setMinutes(Math.floor(start.getMinutes() / boundary) * boundary + boundary)
  if (skipNext) start.setMinutes(start.getMinutes() + intervalMinutes)
  return start
}
export function shouldSkipNext(intervalMinutes: number, now = new Date()): boolean {
  return nextIntervalStart(intervalMinutes, false, now).getTime() - now.getTime() <= 5 * 60 * 1000
}
