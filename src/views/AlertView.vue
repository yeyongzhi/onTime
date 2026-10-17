<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
let removeCloseListener: UnlistenFn | undefined
let disposed = false
onMounted(async () => {
  const unsubscribe = await listen('close-alert-request', async () => {
    closeConfirm.value = true
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
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
const store=useAlarmsStore()
const closeConfirm = ref(false)
async function settle(id:string,minutes:number|null){try{await store.action('settle_occurrence',{id,snoozeMinutes:minutes},minutes === null ? '提醒已停止' : `已延后 ${minutes} 分钟`)}catch{/* toast */}}
async function hideAlertAndContinue() { closeConfirm.value = false; await getCurrentWindow().hide() }
function snoozeLimit(alarmId:string){return store.snapshot?.alarms.find(a=>a.id===alarmId)?.snoozeLimit??0}
</script>
<template>
  <main class="flex min-h-screen flex-col gap-5 p-7">
    <header class="flex items-center gap-3"><AlarmClockIcon class="size-9" /><div><h1 class="text-2xl font-semibold">{{ store.active.length > 1 ? `${store.active.length} 个提醒同时响铃` : '时间到了' }}</h1><p class="text-sm text-muted-foreground">{{ store.active.length > 1 ? '请逐项停止或延后。' : 'onTime 提醒' }}</p></div></header>
    <Card v-for="o in store.active" :key="o.id"><CardHeader><CardTitle>{{ o.title }}</CardTitle><CardDescription>计划 {{ dateLabel(o.scheduledAt) }} · 停止本次不会关闭后续重复提醒</CardDescription></CardHeader><CardContent><Badge variant="secondary">{{ o.reason }} · 贪睡 {{ o.snoozeCount }} / {{ snoozeLimit(o.alarmId) }}</Badge></CardContent><CardFooter class="flex flex-wrap gap-2"><Button @click="settle(o.id,null)" :disabled="store.busy">停止本次</Button><Button v-for="m in [5,10]" :key="m" variant="outline" @click="settle(o.id,m)" :disabled="store.busy||o.snoozeCount>=snoozeLimit(o.alarmId)">{{ m }} 分钟后再响</Button></CardFooter></Card>
    <p v-if="!store.active.length" class="text-muted-foreground">本次提醒已结束。</p>
    <Dialog v-model:open="closeConfirm"><DialogContent><DialogHeader><DialogTitle>关闭提醒窗口？</DialogTitle><DialogDescription>收起窗口后，所有正在响铃的提醒会继续响铃。你可以从系统托盘重新打开 onTime。</DialogDescription></DialogHeader><DialogFooter><Button variant="outline" @click="closeConfirm = false">继续处理提醒</Button><Button @click="hideAlertAndContinue">收起窗口并继续响铃</Button></DialogFooter></DialogContent></Dialog>
  </main>
</template>
