<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { dateLabel, type Occurrence } from '@/lib/types'

const items = ref<Occurrence[]>([])
const busy = ref(false)
let unlisten: UnlistenFn | undefined
let unlistenRefresh: UnlistenFn | undefined
async function refresh() {
  try { items.value = await invoke<Occurrence[]>('get_deferred_reminders') }
  catch (error) { toast.error(String(error)) }
}
onMounted(async () => {
  await refresh()
  unlisten = await listen('close-deferred-summary-request', () => { void acknowledge() })
  unlistenRefresh = await listen('deferred-summary-refresh', () => { void refresh() })
})
onUnmounted(() => { unlisten?.(); unlistenRefresh?.() })
async function acknowledge() {
  if (busy.value) return
  busy.value = true
  try {
    await invoke('acknowledge_deferred_reminders')
    items.value = []
    await getCurrentWindow().hide()
  } catch (error) { toast.error(String(error)) }
  finally { busy.value = false }
}
</script>

<template>
  <main class="flex h-dvh min-h-0 flex-col gap-4 p-6">
    <header>
      <h1 class="text-xl font-semibold">刚才息屏/锁屏的时候错过了以下提醒：</h1>
      <p class="mt-1 text-sm text-muted-foreground">共 {{ items.length }} 条，已记录到提醒历史。</p>
    </header>
    <ScrollArea class="min-h-0 flex-1">
      <ol class="flex list-decimal flex-col gap-2 pl-6">
        <li v-for="item in items" :key="item.id">
          <Card><CardContent class="flex flex-col gap-1 py-3"><span class="break-words font-medium">{{ item.title }}</span><time class="text-sm text-muted-foreground">{{ dateLabel(item.scheduledAt) }}</time></CardContent></Card>
        </li>
      </ol>
    </ScrollArea>
    <Button class="self-end" :disabled="busy" @click="acknowledge">知道了</Button>
  </main>
</template>
