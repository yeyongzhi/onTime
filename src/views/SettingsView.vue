<script setup lang="ts">
import BrandLogo from '@/components/BrandLogo.vue'
import { computed, onMounted, ref, watch } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { storeToRefs } from 'pinia'
import { useUpdatesStore } from '@/stores/updates'
import { getVersion } from '@tauri-apps/api/app'
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart'
import { toast } from 'vue-sonner'
import { useAlarmsStore } from '@/stores/alarms'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { appearance, resetAppearance } from '@/stores/appearance'
import { Button } from '@/components/ui/button'
import { Select, SelectTrigger, SelectValue, SelectContent, SelectGroup, SelectItem } from '@/components/ui/select'

import { Switch } from '@/components/ui/switch'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card'
import { FieldGroup, Field, FieldLabel, FieldDescription } from '@/components/ui/field'


import { ScrollArea } from '@/components/ui/scroll-area'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
const sections = [
  { id: 'appearance', label: '外观' },
  { id: 'general', label: '常规' },
  { id: 'data', label: '数据' },
  { id: 'backup', label: '备份' },
  { id: 'about', label: '关于' },
]
const section = ref('appearance')
const store = useAlarmsStore()
const restoreDialog = ref(false)
const cleanupDialog = ref(false)
const retention = ref('0')
const maintenanceBusy = ref(false)
async function loadRetention() { if (!store.snapshot) return; try { retention.value = String(await invoke<number>('read_retention')) } catch(e) { toast.error(String(e)) } }
async function saveRetention(value: unknown) {
  maintenanceBusy.value = true
  try { await invoke('save_retention', { days: Number(value) }); retention.value = String(value); toast.success('历史保留策略已保存，将在下次每日备份成功后执行') } catch(e) { toast.error(String(e)) } finally { maintenanceBusy.value = false }
}
async function cleanupData() {
  cleanupDialog.value = false; maintenanceBusy.value = true
  try { await store.action('cleanup_data', undefined, undefined).then(message => { if (message) toast.success(String(message)) }) } catch { /* Store reports errors. */ } finally { maintenanceBusy.value = false }
}
const auto = ref(false)
watch(() => store.snapshot?.dataRoot, () => { void loadRetention() }, { immediate: true })
const changing = ref(false)
const version = ref('0.0.1-beta3')
const notificationPermission = ref<'checking' | 'granted' | 'denied' | 'unsupported'>('checking')
const reminderRuntimeStatus = computed(() => {
  if (store.error) return `提醒引擎遇到问题：${store.error}`
  if (store.loading) return '正在检查提醒引擎状态…'
  if (!store.snapshot) return '请选择数据目录后，提醒引擎才能读取并监控提醒。'
  const enabled = store.snapshot.alarms.filter(alarm => alarm.enabled).length
  return enabled ? `提醒引擎正在后台运行，监控 ${enabled} 个已启用提醒。` : '提醒引擎正在后台运行，目前没有已启用的提醒。'
})
const modes = [{ value: 'light', label: '亮色' }, { value: 'dark', label: '暗色' }, { value: 'system', label: '跟随系统' }]
const widths = computed(() => [...new Set([120, 140, 160, 180, 200, 220, 240, 260, 280, 320, appearance.sidebarWidth])].sort((a,b) => a-b))
const radii = computed(() => [...new Set([0, 4, 8, 12, 16, 24, appearance.radius])].sort((a,b) => a-b))
const fonts = computed(() => [...new Set([12, 14, 16, 18, 20, 24, appearance.fontSize])].sort((a,b) => a-b))
function radiusLabel(value: number) { return `${({0: "直角", 4: "微圆角", 8: "小圆角", 12: "中圆角", 16: "大圆角", 24: "超大圆角"} as Record<number,string>)[value] ?? "自定义"} · ${value} px${value === 16 ? "（默认）" : ""}` }
function preference(value: unknown, key: 'fontSize' | 'radius' | 'sidebarWidth') {
  const numeric = Number(value)
  const bounds = { fontSize: [12, 24], radius: [0, 24], sidebarWidth: [120, 320] }[key]
  if (Number.isFinite(numeric) && numeric >= bounds[0]! && numeric <= bounds[1]!) appearance[key] = numeric
}
const updates = useUpdatesStore()
const { update, checkingUpdate, updateError, updateProgress, downloadingUpdate, downloadedInstaller } = storeToRefs(updates)
const { checkUpdate, downloadUpdate } = updates
onMounted(async () => {
  try { auto.value = await isEnabled() } catch { /* Native settings are unavailable in browser previews. */ }
  try { version.value = await getVersion() } catch { /* Keep the package version in browser previews. */ }
  if (!isTauri()) notificationPermission.value = 'unsupported'
  else await checkNotificationPermission()
})
async function checkNotificationPermission() {
  try {
    notificationPermission.value = await invoke<boolean>('plugin:notification|is_permission_granted') ? 'granted' : 'denied'
  } catch { notificationPermission.value = 'unsupported' }
}
async function requestNotificationPermission() {
  try {
    await invoke('plugin:notification|request_permission')
    await checkNotificationPermission()
    if (notificationPermission.value === 'granted') toast.success('系统通知权限已开启')
    else toast.error('系统通知权限未开启，请在系统设置中允许 onTime 通知')
  } catch (e) { toast.error(String(e)) }
}
async function testNotification() {
  try { await invoke('test_notification'); store.notificationError = ''; toast.success('测试通知已发送，请查看系统通知中心') }
  catch (e) { store.notificationError = String(e); toast.error(String(e)) }
}
async function testAudio() {
  try { await invoke('preview_sound', { sound: null, volume: 0.55, stop: false }); toast.success('已发送测试铃声'); }
  catch (e) { store.audioError = String(e); toast.error(String(e)) }
}
function setDisplayMode(value: unknown) { if (value === 'grid' || value === 'list') appearance.displayMode = value }
function mode(value: unknown) { if (value === 'light' || value === 'dark' || value === 'system') appearance.mode = value }

async function changeAuto(value: boolean) {
  changing.value = true
  try { if (value) await enable(); else await disable(); auto.value = await isEnabled(); toast.success('自启动设置已更新') }
  catch (e) { toast.error(String(e)) } finally { changing.value = false }
}
async function backup(command: string) { try { const message = await store.action<string | null>(command); if (message) toast.success(message) } catch { /* Store shows errors. */ } }
async function confirmRestore() { restoreDialog.value = false; await backup('import_backup') }
async function revealInstaller() { if (downloadedInstaller.value) { try { await invoke('reveal_installer', { path: downloadedInstaller.value }) } catch(e) { toast.error(String(e)) } } }
async function migrateData() { try { const result = await store.action('choose_data_root', { migrate: true }); if (result) toast.success('数据已复制并切换，原目录保留') } catch { /* Store reports errors. */ } }
async function relocate() { try { const result = await store.action('choose_data_root'); if (result) toast.success('数据目录已连接') } catch { /* Store shows errors. */ } }
</script>
<template>
  <main class="settings-layout">
    <nav aria-label="设置分类" class="settings-nav">
      <Card size="sm" class="settings-nav-card"><CardHeader><CardTitle><h1>设置</h1></CardTitle></CardHeader><CardContent class="settings-nav-content"><ScrollArea class="settings-menu-scroll"><div class="flex flex-col gap-1">
        <Button v-for="item in sections" :key="item.id" variant="ghost" class="navigation-button settings-category-button" :data-active="section === item.id" :aria-current="section === item.id ? 'page' : undefined" @click="section = item.id"><span>{{ item.label }}</span></Button>
      </div></ScrollArea></CardContent></Card>
    </nav>
    <section class="settings-content" :aria-label="sections.find(item => item.id === section)?.label">
      <Card v-if="section === 'appearance'" class="settings-panel">
        <CardHeader class="flex flex-row flex-wrap items-center gap-3"><div><CardTitle class="settings-panel-heading">外观</CardTitle><CardDescription>统一品牌紫色，调整明暗模式与显示方式。</CardDescription></div><Button variant="secondary" size="sm" class="ml-auto" @click="resetAppearance(); toast.success('外观已恢复默认')">恢复默认</Button></CardHeader>
        <ScrollArea class="settings-panel-scroll"><CardContent><FieldGroup class="appearance-fields">
          <Field class="settings-row"><div class="settings-row-copy"><FieldLabel>外观模式</FieldLabel><FieldDescription>选择亮色、暗色，或跟随系统自动切换。</FieldDescription></div><div class="settings-row-control"><Tabs :model-value="appearance.mode" @update:model-value="mode"><TabsList aria-label="外观模式"><TabsTrigger v-for="item in modes" :key="item.value" :value="item.value">{{ item.label }}</TabsTrigger></TabsList></Tabs></div></Field>
          <Field class="settings-row"><div class="settings-row-copy"><FieldLabel for="default-display-mode">要素默认展示方式</FieldLabel><FieldDescription>统一应用于我的提醒、全部提醒、分组和标签；页面中切换展示方式也会同步更新此设置。</FieldDescription></div><div class="settings-row-control"><Tabs :model-value="appearance.displayMode" @update:model-value="setDisplayMode"><TabsList id="default-display-mode" aria-label="要素默认展示方式"><TabsTrigger value="grid">网格</TabsTrigger><TabsTrigger value="list">列表</TabsTrigger></TabsList></Tabs></div></Field>
          <Field class="settings-row"><div class="settings-row-copy"><FieldLabel for="sidebar-width">侧边栏宽度</FieldLabel></div><div class="settings-row-control"><Select :model-value="String(appearance.sidebarWidth)" @update:model-value="preference($event,'sidebarWidth')"><SelectTrigger id="sidebar-width" class="preference-select"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem v-for="value in widths" :key="value" :value="String(value)">{{ value }} px{{ value === 140 ? '（默认）' : '' }}</SelectItem></SelectGroup></SelectContent></Select></div></Field>
          <Field class="settings-row"><div class="settings-row-copy"><FieldLabel for="corner-radius">全局圆角</FieldLabel></div><div class="settings-row-control"><Select :model-value="String(appearance.radius)" @update:model-value="preference($event,'radius')"><SelectTrigger id="corner-radius" class="preference-select"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem v-for="value in radii" :key="value" :value="String(value)">{{ radiusLabel(value) }}</SelectItem></SelectGroup></SelectContent></Select></div></Field>
          <Field class="settings-row"><div class="settings-row-copy"><FieldLabel for="font-size">全局字体大小</FieldLabel></div><div class="settings-row-control"><Select :model-value="String(appearance.fontSize)" @update:model-value="preference($event,'fontSize')"><SelectTrigger id="font-size" class="preference-select"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem v-for="value in fonts" :key="value" :value="String(value)">{{ value }} px{{ value === 14 ? '（默认）' : '' }}</SelectItem></SelectGroup></SelectContent></Select></div></Field>
        </FieldGroup></CardContent></ScrollArea>
      </Card>
      <Card v-else-if="section === 'general'" class="settings-panel"><CardHeader><CardTitle class="settings-panel-heading">常规</CardTitle><CardDescription>管理启动方式，并检查提醒所需的系统能力。</CardDescription></CardHeader><ScrollArea class="settings-panel-scroll"><CardContent><FieldGroup>
        <Field class="settings-row"><div class="settings-row-copy"><FieldLabel for="autostart">登录时启动 onTime</FieldLabel><FieldDescription>开启后，登录系统时 onTime 会收至托盘并继续监控；手动退出应用后提醒会停止。</FieldDescription></div><div class="settings-row-control"><Switch id="autostart" :model-value="auto" :disabled="changing" @update:model-value="changeAuto" /></div></Field>
        <Field class="settings-row"><div class="settings-row-copy"><FieldLabel>后台提醒状态</FieldLabel><FieldDescription role="status">{{ reminderRuntimeStatus }}</FieldDescription><FieldDescription>应用需要保持在系统托盘运行；系统睡眠期间不会唤醒电脑。恢复运行后，30 分钟内的漏响会补响，更早的提醒会发送摘要并记录在“我的提醒”页。</FieldDescription></div></Field>
        <Field class="settings-row"><div class="settings-row-copy"><FieldLabel>系统通知</FieldLabel><FieldDescription role="status">{{ store.notificationError || (notificationPermission === 'granted' ? '已获系统授权，提醒触发时会发送桌面通知。' : notificationPermission === 'denied' ? '尚未获系统授权，通知可能不会显示。' : notificationPermission === 'unsupported' ? '当前环境无法读取通知权限。' : '正在检查通知权限…') }} 通知横幅也受系统专注模式或勿扰模式影响；应用响铃窗会作为提醒操作入口。</FieldDescription></div><div class="settings-row-control"><div class="flex flex-wrap gap-2"><Button v-if="notificationPermission !== 'granted' && notificationPermission !== 'unsupported'" variant="secondary" @click="requestNotificationPermission">请求通知权限</Button><Button variant="secondary" @click="testNotification">发送测试通知</Button></div></div></Field>
        <Field class="settings-row"><div class="settings-row-copy"><FieldLabel>提醒铃声</FieldLabel><FieldDescription role="status">{{ store.audioError || '可以试听内置铃声，确认系统音频输出正常。' }}</FieldDescription></div><div class="settings-row-control"><div><Button variant="secondary" @click="testAudio">测试铃声</Button></div></div></Field>
      </FieldGroup></CardContent></ScrollArea></Card>
      <Card v-else-if="section === 'data'" class="settings-panel"><CardHeader><CardTitle class="settings-panel-heading">数据</CardTitle><CardDescription>切换会读取所选目录的数据；迁移会复制当前提醒、历史、铃声和设置，成功后切换，原目录保留；迁移前需处理所有响铃和贪睡中的提醒。</CardDescription></CardHeader><ScrollArea class="settings-panel-scroll"><CardContent><FieldGroup>
        <Field class="settings-row"><div class="settings-row-copy"><FieldLabel>数据状态</FieldLabel><FieldDescription role="status">{{ store.error || (store.snapshot ? 'SQLite 数据库已连接，提醒与分组标签已在本机加载。' : '尚未连接本地数据库。') }}</FieldDescription></div></Field>
        <Field class="settings-row"><div class="settings-row-copy"><FieldLabel>数据目录</FieldLabel><FieldDescription class="break-all">{{ store.snapshot?.dataRoot || '尚未选择' }}</FieldDescription></div><div class="settings-row-control"><div><Button variant="secondary" :disabled="store.busy" @click="relocate">{{ store.snapshot ? '切换数据目录' : '选择数据目录' }}</Button><Button v-if="store.snapshot" class="ml-2" variant="secondary" :disabled="store.busy" @click="migrateData">迁移数据</Button></div></div></Field>
      <Field class="settings-row"><div class="settings-row-copy"><FieldLabel>历史保留策略</FieldLabel><FieldDescription>默认永久保留。每日备份成功后清理已结束的旧记录；响铃和贪睡记录保留。</FieldDescription></div><div class="settings-row-control"><Select :model-value="retention" :disabled="maintenanceBusy || !store.snapshot" @update:model-value="saveRetention"><SelectTrigger class="preference-select"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="0">永久保留</SelectItem><SelectItem v-for="days in [30,90,180,365]" :key="days" :value="String(days)">保留 {{ days }} 天</SelectItem></SelectGroup></SelectContent></Select></div></Field><Field class="settings-row"><div class="settings-row-copy"><FieldLabel>数据清理</FieldLabel><FieldDescription>按策略清理历史，并删除未被已保存提醒引用的铃声。请先保存正在编辑的提醒。</FieldDescription></div><div class="settings-row-control"><Button variant="secondary" :disabled="maintenanceBusy || store.busy || !store.snapshot" @click="cleanupDialog = true">清理历史与未使用铃声</Button></div></Field></FieldGroup></CardContent></ScrollArea></Card>
      <Card v-else-if="section === 'backup'" class="settings-panel"><CardHeader><CardTitle class="settings-panel-heading">备份与恢复</CardTitle><CardDescription>导出或恢复本地业务数据。</CardDescription></CardHeader><ScrollArea class="settings-panel-scroll"><CardContent><FieldGroup><Field class="settings-row"><div class="settings-row-copy"><FieldLabel>备份与恢复</FieldLabel><FieldDescription>onTime 每天首次启动后自动备份 SQLite 数据库，保留最近 7 份，存放在数据目录的 backups 文件夹。手动导出包含提醒、分组、标签、提醒历史和铃声；设置项单独保存在本机。恢复会覆盖当前业务数据，并保留恢复前备份。</FieldDescription></div><div class="settings-row-control flex flex-wrap gap-2"><Button variant="secondary" :disabled="store.pending.includes('export_backup') || !store.snapshot" @click="backup('export_backup')">导出数据备份</Button><Button variant="secondary" :disabled="store.busy || !store.snapshot" @click="restoreDialog = true">恢复备份</Button></div></Field></FieldGroup></CardContent></ScrollArea></Card>
      <Card v-else class="settings-panel">
        <CardHeader><CardTitle class="settings-panel-heading">关于 onTime</CardTitle><CardDescription>版本 {{ version }}</CardDescription></CardHeader>
        <ScrollArea class="settings-panel-scroll"><CardContent><div class="settings-about-row"><div class="flex min-w-0 flex-col gap-4">
          <div class="flex items-center gap-4"><BrandLogo class="size-16" /><div><h2 class="text-xl font-semibold">onTime</h2><p class="text-sm text-muted-foreground">轻量桌面提醒工具</p><p class="mt-1 text-sm text-muted-foreground">要紧的事，到点提醒。</p></div></div>
          <p v-if="checkingUpdate" role="status" aria-live="polite">正在检查稳定版和预览版…</p><p v-if="updateError" role="alert" class="text-sm text-destructive">{{ updateError }}</p>
          <p v-if="update?.latest" class="text-sm text-muted-foreground">最新版本 {{ update.latest }}</p>
          <p v-if="update" role="status" class="text-sm">{{ update.message }}</p>

          <p v-if="downloadingUpdate" class="text-xs text-muted-foreground" role="status" aria-live="polite">正在下载更新{{ updateProgress !== null ? `：${updateProgress}%` : '…' }}</p>
          <p v-if="downloadedInstaller" class="break-all text-xs text-muted-foreground" role="status">安装包已保存至：{{ downloadedInstaller }}</p>
          <section v-if="update?.available && update.notes" class="flex flex-col gap-2">
            <h2 class="text-sm font-semibold">更新内容</h2>
            <pre class="whitespace-pre-wrap break-words text-sm text-muted-foreground">{{ update.notes }}</pre>
          </section>
        </div><div class="settings-row-control">          <div class="flex flex-wrap items-center gap-2">
            <Button variant="secondary" :disabled="checkingUpdate || downloadingUpdate" @click="checkUpdate">{{ checkingUpdate ? '检查中…' : updateError ? '重试检查' : '检查更新' }}</Button>
            <Button v-if="update?.available" variant="secondary" :disabled="downloadingUpdate" @click="downloadUpdate">{{ downloadingUpdate ? updateProgress === null ? '下载中…' : `下载中 ${updateProgress}%` : downloadedInstaller ? '重新下载安装包' : '下载安装包' }}</Button>
            <Button v-if="downloadedInstaller" variant="secondary" @click="revealInstaller">打开所在文件夹</Button>
          </div></div></div></CardContent></ScrollArea>
      </Card>
    </section>
    <Dialog v-model:open="cleanupDialog"><DialogContent><DialogHeader><DialogTitle>清理历史和未使用铃声？</DialogTitle><DialogDescription>先备份业务数据，再按所选保留策略清理已结束的历史，并删除未被已保存提醒引用的铃声。此操作不会清理备份文件。</DialogDescription></DialogHeader><DialogFooter><Button variant="secondary" @click="cleanupDialog = false">取消</Button><Button variant="destructive" @click="cleanupData">备份并清理</Button></DialogFooter></DialogContent></Dialog>
    <Dialog v-model:open="restoreDialog"><DialogContent><DialogHeader><DialogTitle>覆盖恢复当前数据？</DialogTitle><DialogDescription>恢复会替换提醒、标签和记录，并取消当前及备份中的活动提醒，避免恢复后旧提醒再次响起。应用会保留恢复前备份；取消文件选择不会修改数据。</DialogDescription></DialogHeader><DialogFooter><Button variant="secondary" @click="restoreDialog = false">取消</Button><Button variant="destructive" :disabled="store.busy" @click="confirmRestore">选择备份并恢复</Button></DialogFooter></DialogContent></Dialog>
  </main>
</template>
