import { ref } from 'vue'
import { defineStore } from 'pinia'
import { listen } from '@tauri-apps/api/event'
import { toast } from 'vue-sonner'
import { useAlarmsStore } from '@/stores/alarms'
interface UpdateInfo { current: string; latest: string | null; available: boolean; notes: string; url: string | null; message: string }
export const useUpdatesStore = defineStore('updates', () => {
  const alarms = useAlarmsStore()
  const update = ref<UpdateInfo | null>(null)
  const checkingUpdate = ref(false)
  const updateError = ref('')
  const updateProgress = ref<number | null>(null)
  const downloadingUpdate = ref(false)
  const downloadedInstaller = ref('')
  async function checkUpdate() {
    if (checkingUpdate.value || downloadingUpdate.value) return
    checkingUpdate.value = true; updateError.value = ''; update.value = null
    try { update.value = await alarms.action<UpdateInfo>('check_update') ?? null }
    catch(e) { updateError.value = String(e) }
    finally { checkingUpdate.value = false }
  }
  async function downloadUpdate() {
    if (!update.value?.url || downloadingUpdate.value) return
    downloadingUpdate.value = true; updateProgress.value = 0; downloadedInstaller.value = ''; updateError.value = ''
    try {
      const unlisten = await listen<{ percent: number | null }>('update-download-progress', event => {
        updateProgress.value = event.payload.percent === null ? null : Math.min(99, event.payload.percent)
      })
      try {
        const path = await alarms.action<string | null>('download_update', { url: update.value.url })
        if (path) { downloadedInstaller.value = path; updateProgress.value = 100; toast.success('安装包已下载') }
        else updateProgress.value = null
      } finally { unlisten() }
    } catch(e) { updateProgress.value = null; updateError.value = String(e) }
    finally { downloadingUpdate.value = false }
  }
  return { update, checkingUpdate, updateError, updateProgress, downloadingUpdate, downloadedInstaller, checkUpdate, downloadUpdate }
})
