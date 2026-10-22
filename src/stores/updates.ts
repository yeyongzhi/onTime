import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import { listen } from '@tauri-apps/api/event'
import { toast } from 'vue-sonner'
import { useAlarmsStore } from '@/stores/alarms'
interface UpdateInfo { current: string; latest: string | null; available: boolean; notes: string; url: string | null; message: string; sha256: string | null; checkedAt: number; cached: boolean; warning: string | null }
export const useUpdatesStore = defineStore('updates', () => {
  const alarms = useAlarmsStore()
  const update = ref<UpdateInfo | null>(null)
  const checkingUpdate = ref(false)
  const updateError = ref('')
  const updateProgress = ref<number | null>(null)
  const downloadingUpdate = ref(false)
  const downloadedInstaller = ref('')
  const retryAt = ref(0)
  const retryIn = computed(() => Math.max(0, retryAt.value - alarms.now))
  async function checkUpdate(manual = true) {
    if (checkingUpdate.value || downloadingUpdate.value || (manual && retryIn.value > 0)) return
    checkingUpdate.value = true; updateError.value = ''
    if (manual) retryAt.value = Math.floor(Date.now() / 1000) + 30
    try {
      const result = await invoke<UpdateInfo>('check_update', { manual })
      if (result) { update.value = result; updateError.value = result.warning ?? '' }
    } catch(e) { updateError.value = String(e) }
    finally { checkingUpdate.value = false }
  }
  async function openReleases() {
    try { await invoke('open_releases') } catch(e) { toast.error(String(e)) }
  }
  async function downloadUpdate() {
    if (!update.value?.url || !update.value.sha256 || downloadingUpdate.value) return
    downloadingUpdate.value = true; updateProgress.value = 0; downloadedInstaller.value = ''; updateError.value = ''
    try {
      const unlisten = await listen<{ percent: number | null }>('update-download-progress', event => {
        updateProgress.value = event.payload.percent === null ? null : Math.min(99, event.payload.percent)
      })
      try {
        const path = await alarms.action<string | null>('download_update', { url: update.value.url, sha256: update.value.sha256 })
        if (path) { downloadedInstaller.value = path; updateProgress.value = 100; toast.success('安装包已下载并通过校验') }
        else updateProgress.value = null
      } finally { unlisten() }
    } catch(e) { updateProgress.value = null; updateError.value = String(e) }
    finally { downloadingUpdate.value = false }
  }
  return { update, checkingUpdate, updateError, updateProgress, downloadingUpdate, downloadedInstaller, checkUpdate, downloadUpdate, retryIn, openReleases }
})
