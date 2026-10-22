import { test } from 'node:test'
import assert from 'node:assert/strict'
import { nextIntervalStart, shouldSkipNext } from '../src/lib/schedule.ts'
const at = (time) => new Date(`2026-10-22T${time}`)
test('半小时从下个整点或半点开始，整点本身取下一点', () => {
  for (const [now, next] of [['09:29:00','09:30:00'],['09:30:00','10:00:00'],['09:59:59','10:00:00']])
    assert.equal(nextIntervalStart(30, false, at(now)).getTime(), at(next).getTime())
})
test('1 / 2 小时从下个整点开始，跳过首次点按所选间隔延后', () => {
  for (const interval of [60, 120]) {
    assert.equal(nextIntervalStart(interval, false, at('09:29:00')).getTime(), at('10:00:00').getTime())
    assert.equal(nextIntervalStart(interval, true, at('09:59:00')).getTime(), at(interval === 60 ? '11:00:00' : '12:00:00').getTime())
  }
})
test('5 分钟阈值含边界，29 / 59 分钟默认跳过，跨天不丢日期', () => {
  assert.equal(shouldSkipNext(30, at('09:24:59')), false)
  assert.equal(shouldSkipNext(30, at('09:25:00')), true)
  assert.equal(shouldSkipNext(30, at('09:29:00')), true)
  assert.equal(shouldSkipNext(60, at('09:59:00')), true)
  assert.equal(shouldSkipNext(120, at('09:29:00')), false)
  assert.equal(nextIntervalStart(30, true, at('23:59:00')).getTime(), new Date('2026-10-23T00:30:00').getTime())
})
