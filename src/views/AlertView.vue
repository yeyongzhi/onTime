<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
let removeCloseListener: UnlistenFn | undefined
let disposed = false
onMounted(async () => {
  const unsubscribe = await listen('close-alert-request', async () => {
    if (store.busy) return
    for (const occurrence of [...store.active]) await settle(occurrence.id, null)
  })
  if (disposed) unsubscribe()
  else removeCloseListener = unsubscribe
})
onUnmounted(() => { disposed = true; removeCloseListener?.() })
import { AlarmClockIcon } from '@lucide/vue'
import { useAlarmsStore } from '@/stores/alarms'
import { dateLabel } from '@/lib/types'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Card,CardHeader,CardTitle,CardDescription,CardContent,CardFooter } from '@/components/ui/card'
const store=useAlarmsStore()
async function settle(id:string,minutes:number|null){try{await store.action('settle_occurrence',{id,snoozeMinutes:minutes})}catch{/* toast */}}
function snoozeLimit(alarmId:string){return store.snapshot?.alarms.find(a=>a.id===alarmId)?.snoozeLimit??0}
</script>
<template><main class="flex min-h-screen flex-col gap-5 p-7"><header class="flex items-center gap-3"><AlarmClockIcon class="size-9" /><div><h1 class="text-2xl font-semibold">时间到了</h1><p class="text-sm text-muted-foreground">onTime 提醒</p></div></header><Card v-for="o in store.active" :key="o.id"><CardHeader><CardTitle>{{ o.title }}</CardTitle><CardDescription>计划 {{ dateLabel(o.scheduledAt) }}</CardDescription></CardHeader><CardContent><Badge variant="secondary">{{ o.reason }} · 贪睡 {{ o.snoozeCount }} / {{ snoozeLimit(o.alarmId) }}</Badge></CardContent><CardFooter class="flex flex-wrap gap-2"><Button @click="settle(o.id,null)" :disabled="store.busy">停止提醒</Button><Button v-for="m in [5,10]" :key="m" variant="outline" @click="settle(o.id,m)" :disabled="store.busy||o.snoozeCount>=snoozeLimit(o.alarmId)">{{ m }} 分钟后再响</Button></CardFooter></Card><p v-if="!store.active.length" class="text-muted-foreground">本次提醒已结束。</p></main></template>