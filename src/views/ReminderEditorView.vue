<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import AlarmEditor from '@/components/AlarmEditor.vue'
import { useAlarmsStore } from '@/stores/alarms'
import { newAlarm } from '@/lib/types'
import { Button } from '@/components/ui/button'
import { Empty, EmptyHeader, EmptyTitle, EmptyDescription, EmptyContent } from '@/components/ui/empty'
const route = useRoute()
const router = useRouter()
const store = useAlarmsStore()
function back() {
  const from = route.query.from
  router.push(typeof from === 'string' && from.startsWith('/') ? from : '/reminders')
}
const alarm = computed(() => {
  const id = String(route.params.id ?? route.query.copy ?? '')
  if (!id) {
    const value = newAlarm()
    const groupId = String(route.query.group ?? '')
    if (store.snapshot?.groups.some(g => g.id === groupId)) value.groupId = groupId
    return value
  }
  const source = store.snapshot?.alarms.find(a => a.id === id)
  if (!source) return null
  const value = JSON.parse(JSON.stringify(source))
  if (route.query.copy) { value.id = ''; value.title = `${value.title}（副本）`.slice(0, 80); value.enabled = false }
  return value
})
</script>
<template>
  <AlarmEditor v-if="store.snapshot && alarm" :key="route.fullPath" :alarm="alarm" @close="back()" />
  <main v-else class="workspace-page"><Empty><EmptyHeader><EmptyTitle>{{ store.loading ? '正在读取提醒…' : '暂时无法打开提醒' }}</EmptyTitle><EmptyDescription>{{ store.error || '请返回列表检查数据目录或提醒是否存在。' }}</EmptyDescription></EmptyHeader><EmptyContent><Button variant="outline" @click="back()">返回提醒列表</Button></EmptyContent></Empty></main>
</template>
