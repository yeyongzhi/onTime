import { test } from 'node:test'
import assert from 'node:assert/strict'
import { alarmEffective, newAlarm } from '../src/lib/types.ts'

const now = new Date('2026-10-20T12:00:00').getTime() / 1000
test('停用重复提醒仍然有效，过期有效期独立于开关', () => {
  const alarm = { ...newAlarm(), enabled: false, repeat: 'daily' }
  assert.equal(alarmEffective(alarm, now), true)
  assert.equal(alarmEffective({ ...alarm, effectiveEnd: '2026-10-19' }, now), false)
  assert.equal(alarmEffective({ ...alarm, effectiveEnd: '2026-10-20' }, now), true)
})
test('单次提醒到期后失效，启用开关不参与生效判断', () => {
  for (const enabled of [true, false]) {
    const alarm = { ...newAlarm(), enabled, repeat: 'once', date: '2026-10-20' }
    assert.equal(alarmEffective({ ...alarm, time: '12:01' }, now), true)
    assert.equal(alarmEffective({ ...alarm, time: '12:00' }, now), false)
    assert.equal(alarmEffective({ ...alarm, time: '11:59' }, now), false)
  }
})
test('倒计时到期后失效，未启动的倒计时保持有效', () => {
  const alarm = { ...newAlarm(), kind: 'timer' }
  assert.equal(alarmEffective({ ...alarm, deadline: null, enabled: false }, now), true)
  assert.equal(alarmEffective({ ...alarm, deadline: now + 1 }, now), true)
  assert.equal(alarmEffective({ ...alarm, deadline: now }, now), false)
})
