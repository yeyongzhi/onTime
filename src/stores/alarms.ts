import { loadAppearance } from '@/stores/appearance'
import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { toast } from 'vue-sonner'
import type { Snapshot } from '@/lib/types'
export const useAlarmsStore = defineStore('alarms', () => {
  const snapshot = ref<Snapshot | null>(null)
  const loading = ref(true)
  const error = ref('')
  const busy = ref(false)
  const now = ref(Math.floor(Date.now()/1000))
  const active = computed(() => snapshot.value?.occurrences.filter(o => o.status === 'ringing') ?? [])
  let unlisten: UnlistenFn[] = []
  let timer: ReturnType<typeof setInterval> | undefined
  let refreshPending: Promise<void> | undefined
  async function refresh() {
    if (refreshPending) return refreshPending
    refreshPending = (async () => {
      try { snapshot.value = await invoke<Snapshot | null>('get_snapshot'); error.value = '' }
      catch (e) { error.value = String(e) }
      finally { loading.value = false; refreshPending = undefined }
    })()
    return refreshPending
  }
  async function start() {
    if (!isTauri()) { error.value = '请在 onTime 桌面应用中使用；浏览器不执行后台提醒。'; loading.value = false; return }
    unlisten = await Promise.all([
      listen('state-changed', () => void refresh()),
      listen<string>('storage-error', e => { error.value = e.payload }),
      listen<string>('audio-error', e => toast.error(e.payload)),
    ])
    await refresh()
    timer = setInterval(() => { now.value = Math.floor(Date.now()/1000) }, 1000)
  }
  function stop() { unlisten.forEach(f => f()); unlisten = []; if(timer) clearInterval(timer) }
  async function action<T = void>(command: string, args?: Record<string, unknown>, success?: string): Promise<T | undefined> {
    if (busy.value) return
    busy.value = true
    try { const result = await invoke<T>(command, args); await refresh(); if (command === 'choose_data_root' || command === 'import_backup') await loadAppearance(); if(success) toast.success(success); return result }
    catch(e) { toast.error(String(e)); throw e }
    finally { busy.value = false }
  }
  return { snapshot, loading, error, busy, now, active, start, stop, refresh, action }
})
