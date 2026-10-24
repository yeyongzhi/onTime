import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import { listen } from '@tauri-apps/api/event'
import { toast } from 'vue-sonner'
import router from '@/router'
import { isNavigationFailure, NavigationFailureType } from 'vue-router'
import { useAlarmsStore } from '@/stores/alarms'
interface UpdateInfo { current: string; latest: string | null; available: boolean; notes: string; url: string | null; message: string; sha256: string | null; checkedAt: number; cached: boolean; warning: string | null; autoInstall: boolean }
export const useUpdatesStore = defineStore('updates', () => {
  const alarms = useAlarmsStore()
  const update = ref<UpdateInfo | null>(null)
  const checkingUpdate = ref(false)
  const updateError = ref('')
  const installResultMessage = ref('')
  const updateProgress = ref<number | null>(null)
  const downloadingUpdate = ref(false)
  const downloadedInstaller = ref('')
  const installingUpdate = ref(false)
  let preparingInstall = false
  async function checkUpdate(manual = true) {
    if (checkingUpdate.value || downloadingUpdate.value || installingUpdate.value) return
    checkingUpdate.value = true; updateError.value = ''
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
    if (!update.value?.url || !update.value.sha256 || downloadingUpdate.value || installingUpdate.value) return
    const automatic = update.value.autoInstall
    downloadingUpdate.value = true; updateProgress.value = 0; downloadedInstaller.value = ''; updateError.value = ''
    try {
      const unlisten = await listen<{ percent: number | null }>('update-download-progress', event => {
        updateProgress.value = event.payload.percent === null ? null : Math.min(99, event.payload.percent)
      })
      try {
        const path = await alarms.action<string | null>('download_update', { url: update.value.url, sha256: update.value.sha256 })
        if (path) { downloadedInstaller.value = path; updateProgress.value = 100; if (automatic) await installUpdate(); else toast.success('安装包已下载并通过校验') }
        else updateProgress.value = null
      } finally { unlisten() }
    } catch(e) { updateProgress.value = null; updateError.value = String(e) }
    finally { downloadingUpdate.value = false }
  }
  async function installUpdate() {
    if (!downloadedInstaller.value || installingUpdate.value || preparingInstall) return
    if (alarms.busy) { updateError.value = '请等待当前数据操作完成，再点击安装并重启。'; return }
    preparingInstall = true; updateError.value = ''; installResultMessage.value = ''
    try {
      const failure = await router.push('/settings')
      if (isNavigationFailure(failure, NavigationFailureType.aborted | NavigationFailureType.cancelled)) {
        updateError.value = '更新已下载。请先保存或放弃当前编辑，再到关于页点击“安装并重启”。'
        return
      }
      installingUpdate.value = true; alarms.busy = true
      await invoke('install_downloaded_update')
    } catch(e) { updateError.value = String(e); toast.error(String(e)) }
    finally { if (installingUpdate.value) alarms.busy = false; installingUpdate.value = false; preparingInstall = false }
  }
  async function showInstallResult() {
    try {
      const result = await invoke<{ success: boolean; message: string } | null>('take_update_result')
      if (result) { if (result.success) toast.success(result.message); else { installResultMessage.value = result.message; toast.error(result.message) } }
    } catch { /* A missing receipt must not interrupt reminders or startup. */ }
  }
  return { update, checkingUpdate, updateError, installResultMessage, updateProgress, downloadingUpdate, downloadedInstaller, checkUpdate, downloadUpdate, openReleases, installingUpdate, installUpdate, showInstallResult }
})
