<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalSize } from '@tauri-apps/api/dpi'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
let removeCloseListener: UnlistenFn | undefined
let disposed = false
const alertHeader = ref<HTMLElement | null>(null)
const alertContent = ref<HTMLElement | null>(null)
let sizeObserver: ResizeObserver | undefined
let lastHeight = 0
async function fitWindow() {
  if (!alertHeader.value || !alertContent.value || disposed) return
  const height = Math.min(Math.max(240, Math.ceil(alertHeader.value.getBoundingClientRect().height + alertContent.value.getBoundingClientRect().height + 72)), Math.max(240, window.screen.availHeight - 80))
  if (height === lastHeight) return
  lastHeight = height
  try { await getCurrentWindow().setSize(new LogicalSize(540, height)) } catch { lastHeight = 0 }
}
onMounted(async () => {
  sizeObserver = new ResizeObserver(() => { void fitWindow() })
  if (alertHeader.value) sizeObserver.observe(alertHeader.value)
  if (alertContent.value) sizeObserver.observe(alertContent.value)
  void fitWindow()
  const unsubscribe = await listen('close-alert-request', async () => {
    closeConfirm.value = true
  })
  if (disposed) unsubscribe()
  else removeCloseListener = unsubscribe
})
onUnmounted(() => { disposed = true; sizeObserver?.disconnect(); removeCloseListener?.() })
import { AlarmClockIcon, CheckIcon, Clock3Icon } from '@lucide/vue'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useAlarmsStore } from '@/stores/alarms'
import { dateLabel } from '@/lib/types'
import { Button } from '@/components/ui/button'
import { Card,CardHeader,CardTitle,CardContent,CardFooter } from '@/components/ui/card'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
const store=useAlarmsStore()
const closeConfirm = ref(false)
async function settle(id:string,minutes:number|null){try{await store.action('settle_occurrence',{id,snoozeMinutes:minutes},minutes === null ? '提醒已停止' : `已延后 ${minutes} 分钟`)}catch{/* toast */}}
async function hideAlertAndContinue() { closeConfirm.value = false; await getCurrentWindow().hide() }
function snoozeLimit(alarmId:string){return store.snapshot?.alarms.find(a=>a.id===alarmId)?.snoozeLimit??0}
</script>
<template>
  <main class="alert-page">
    <header ref="alertHeader" class="flex shrink-0 items-center gap-3"><div class="alert-emblem"><AlarmClockIcon /></div><div><h1 class="text-2xl font-semibold">{{ store.active.length > 1 ? `${store.active.length} 个提醒同时响铃` : '时间到了' }}</h1><p class="text-sm text-muted-foreground">{{ store.active.length > 1 ? '请逐项停止或延后。' : '先停下来，处理这件重要的小事。' }}</p></div></header>
    <ScrollArea class="min-h-0 flex-1"><div ref="alertContent" class="flex flex-col gap-4"><Card v-for="o in store.active" :key="o.id"><CardHeader class="alert-reminder-header"><CardTitle class="alert-title">{{ o.title }}</CardTitle><div class="alert-time"><span class="text-sm text-muted-foreground">提醒时间</span><time :datetime="new Date((o.snoozeUntil ?? o.scheduledAt) * 1000).toISOString()" class="font-medium tabular-nums">{{ dateLabel(o.snoozeUntil ?? o.scheduledAt) }}</time></div></CardHeader><CardContent v-if="store.snapshot?.alarms.find(a => a.id === o.alarmId)?.content?.trim()"><p class="whitespace-pre-wrap break-words text-base">{{ store.snapshot?.alarms.find(a => a.id === o.alarmId)?.content }}</p></CardContent><CardFooter class="alert-actions"><Button class="alert-stop" @click="settle(o.id,null)" :disabled="store.busy"><CheckIcon data-icon="inline-start" />停止本次提醒</Button><div class="grid grid-cols-2 gap-3"><Button v-for="m in [5,10]" :key="m" class="alert-snooze" variant="secondary" @click="settle(o.id,m)" :disabled="store.busy||o.snoozeCount>=snoozeLimit(o.alarmId)"><Clock3Icon data-icon="inline-start" />{{ m }} 分钟后再响</Button></div><p class="text-center text-sm text-muted-foreground">{{ o.snoozeCount >= snoozeLimit(o.alarmId) ? '已达到本次贪睡次数上限' : '停止本次提醒，后续重复提醒仍会照常响铃' }}</p></CardFooter></Card>
    <p v-if="!store.active.length" class="text-muted-foreground">本次提醒已结束。</p>
    </div></ScrollArea><Dialog v-model:open="closeConfirm"><DialogContent><DialogHeader><DialogTitle>关闭提醒窗口？</DialogTitle><DialogDescription>收起窗口后，所有正在响铃的提醒会继续响铃。你可以从系统托盘重新打开 onTime。</DialogDescription></DialogHeader><DialogFooter><Button variant="secondary" @click="closeConfirm = false">继续处理提醒</Button><Button @click="hideAlertAndContinue">收起窗口并继续响铃</Button></DialogFooter></DialogContent></Dialog>
  </main>
</template>

<style scoped>
.alert-page { height: 100dvh; display: flex; flex-direction: column; gap: 24px; padding: 24px; overflow: hidden; }
.alert-emblem { display: grid; place-items: center; width: 64px; height: 64px; flex-shrink: 0; border-radius: 20px; background: var(--primary); color: var(--primary-foreground); }
.alert-emblem svg { width: 34px; height: 34px; }
.alert-reminder-header { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; padding-block: 8px; }
.alert-time { display: flex; flex-shrink: 0; flex-direction: column; align-items: flex-end; gap: 4px; }
.alert-title { flex: 1; min-width: 0; font-size: 1.6rem; line-height: 1.3; overflow-wrap: anywhere; }
.alert-actions { display: flex; flex-direction: column; align-items: stretch; gap: 12px; padding: 20px; }
.alert-stop { min-height: 60px; height: auto; font-size: 1.2rem; font-weight: 600; }
.alert-snooze { min-height: 48px; height: auto; font-size: 1rem; }
</style>
