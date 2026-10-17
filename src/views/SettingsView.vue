<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getVersion } from '@tauri-apps/api/app'
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart'
import { toast } from 'vue-sonner'
import { useAlarmsStore } from '@/stores/alarms'
import { appearance, resetAppearance } from '@/stores/appearance'
import { Button } from '@/components/ui/button'
import { Select, SelectTrigger, SelectValue, SelectContent, SelectGroup, SelectItem } from '@/components/ui/select'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card'
import { FieldGroup, Field, FieldLabel, FieldDescription } from '@/components/ui/field'

import { Questionnaire, QuestionnaireItem, QuestionnaireTitle, QuestionnaireChoices, QuestionnaireChoice } from '@/components/ui/questionnaire'

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
const auto = ref(false)
const changing = ref(false)
const version = ref('0.1.0')
const notificationPermission = ref<'checking' | 'granted' | 'denied' | 'unsupported'>('checking')
const reminderRuntimeStatus = computed(() => {
  if (store.error) return `提醒引擎遇到问题：${store.error}`
  if (store.loading) return '正在检查提醒引擎状态…'
  if (!store.snapshot) return '请选择数据目录后，提醒引擎才能读取并监控提醒。'
  const enabled = store.snapshot.alarms.filter(alarm => alarm.enabled).length
  return enabled ? `提醒引擎正在后台运行，监控 ${enabled} 个已启用提醒。` : '提醒引擎正在后台运行，目前没有已启用的提醒。'
})
const colors = [{ value: '#f43f5e', label: '红' }, { value: '#f97316', label: '橙' }, { value: '#eab308', label: '黄' }, { value: '#00b954', label: '绿' }, { value: '#3b82f6', label: '蓝' }, { value: '#6366f1', label: '靛' }, { value: '#a855f7', label: '紫' }]
const customColorDialog = ref(false)
const customColorSelected = ref(!colors.some(item => item.value === appearance.color))
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
function color(value: unknown) { if (typeof value === 'string' && /^#[\da-f]{6}$/i.test(value)) appearance.color = value }
interface UpdateInfo { current: string; latest: string | null; available: boolean; notes: string; url: string | null; message: string }
const update = ref<UpdateInfo | null>(null)
const updateProgress = ref<number | null>(null)
const downloadingUpdate = ref(false)
const downloadedInstaller = ref('')
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
function mode(value: unknown) { if (value === 'light' || value === 'dark' || value === 'system') appearance.mode = value }

async function changeAuto(value: boolean) {
  changing.value = true
  try { if (value) await enable(); else await disable(); auto.value = await isEnabled(); toast.success('自启动设置已更新') }
  catch (e) { toast.error(String(e)) } finally { changing.value = false }
}
async function backup(command: string) { try { const message = await store.action<string | null>(command); if (message) toast.success(message) } catch { /* Store shows errors. */ } }
async function confirmRestore() { restoreDialog.value = false; await backup('import_backup') }
async function relocate() { try { const result = await store.action('choose_data_root'); if (result) toast.success('数据目录已连接') } catch { /* Store shows errors. */ } }
async function checkUpdate() { try { const result = await store.action<UpdateInfo>('check_update'); if (result) update.value = result } catch { /* Store shows errors. */ } }
async function downloadUpdate() {
  if (!update.value?.url) return
  updateProgress.value = 0
  downloadedInstaller.value = ''
  downloadingUpdate.value = true
  try {
    const unlisten = await listen<{ percent: number | null }>('update-download-progress', event => { updateProgress.value = event.payload.percent })
    try {
      const path = await store.action<string | null>('download_update', { url: update.value.url })
      if (path) { downloadedInstaller.value = path; updateProgress.value = 100; toast.success('安装包已下载') }
      else updateProgress.value = null
    } finally { unlisten() }
  } catch { updateProgress.value = null /* Store shows errors. */ }
  finally { downloadingUpdate.value = false }
}
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
        <CardHeader class="flex flex-row flex-wrap items-center gap-3"><div><CardTitle class="settings-panel-heading">外观</CardTitle><CardDescription>调整应用的颜色与显示方式。</CardDescription></div><Button variant="ghost" size="sm" class="ml-auto" @click="resetAppearance(); customColorSelected = false; toast.success('外观已恢复默认')">恢复默认</Button></CardHeader>
        <ScrollArea class="settings-panel-scroll"><CardContent><FieldGroup class="appearance-fields">
          <Questionnaire :items="[{ name: 'appearance-mode', required: true }]" @submit.prevent><QuestionnaireItem name="appearance-mode" required><QuestionnaireTitle>外观模式</QuestionnaireTitle><QuestionnaireChoices class="grid grid-cols-3 gap-2"><QuestionnaireChoice v-for="item in modes" :key="item.value" :value="item.value" :checked="appearance.mode === item.value" @update:checked="checked => { if (checked) mode(item.value) }">{{ item.label }}</QuestionnaireChoice></QuestionnaireChoices></QuestionnaireItem></Questionnaire>
          <Questionnaire :items="[{ name: 'theme-color', required: true }]" @submit.prevent>
            <QuestionnaireItem name="theme-color" required>
              <QuestionnaireTitle>主题色</QuestionnaireTitle>
              <QuestionnaireChoices class="flex flex-wrap gap-2">
                <QuestionnaireChoice v-for="item in colors" :key="item.value" class="color-choice" :value="item.value" :checked="!customColorSelected && appearance.color === item.value" @update:checked="checked => { if (checked) { customColorSelected = false; color(item.value) } }"><span class="size-4 rounded-full" :style="{ backgroundColor: item.value }" /><span>{{ item.label }}</span></QuestionnaireChoice>
                <QuestionnaireChoice class="color-choice" value="custom" :checked="customColorSelected" @click="customColorDialog = true" @update:checked="checked => { if (checked) customColorSelected = true }"><span class="size-4 rounded-full" :style="{ backgroundColor: appearance.color }" /><span>自定义</span></QuestionnaireChoice>
              </QuestionnaireChoices>
            </QuestionnaireItem>
          </Questionnaire>
          <Dialog v-model:open="customColorDialog"><DialogContent class="sm:max-w-sm"><DialogHeader><DialogTitle>自定义主题色</DialogTitle><DialogDescription>选择颜色后即时应用。</DialogDescription></DialogHeader><FieldGroup><Field><FieldLabel for="theme-color">颜色</FieldLabel><Input id="theme-color" :model-value="appearance.color" @update:model-value="value => { customColorSelected = true; color(value) }" type="color" aria-label="自定义主题色" class="w-full" /></Field></FieldGroup><DialogFooter><Button @click="customColorDialog = false">完成</Button></DialogFooter></DialogContent></Dialog>
          <Field><FieldLabel for="sidebar-width">侧边栏宽度</FieldLabel><Select :model-value="String(appearance.sidebarWidth)" @update:model-value="preference($event,'sidebarWidth')"><SelectTrigger id="sidebar-width" class="preference-select"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem v-for="value in widths" :key="value" :value="String(value)">{{ value }} px{{ value === 140 ? '（默认）' : '' }}</SelectItem></SelectGroup></SelectContent></Select></Field>
          <Field><FieldLabel for="corner-radius">全局圆角</FieldLabel><Select :model-value="String(appearance.radius)" @update:model-value="preference($event,'radius')"><SelectTrigger id="corner-radius" class="preference-select"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem v-for="value in radii" :key="value" :value="String(value)">{{ radiusLabel(value) }}</SelectItem></SelectGroup></SelectContent></Select></Field>
          <Field><FieldLabel for="font-size">全局字体大小</FieldLabel><Select :model-value="String(appearance.fontSize)" @update:model-value="preference($event,'fontSize')"><SelectTrigger id="font-size" class="preference-select"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem v-for="value in fonts" :key="value" :value="String(value)">{{ value }} px{{ value === 14 ? '（默认）' : '' }}</SelectItem></SelectGroup></SelectContent></Select></Field>
        </FieldGroup></CardContent></ScrollArea>
      </Card>
      <Card v-else-if="section === 'general'" class="settings-panel"><CardHeader><CardTitle class="settings-panel-heading">常规</CardTitle><CardDescription>管理启动方式，并检查提醒所需的系统能力。</CardDescription></CardHeader><ScrollArea class="settings-panel-scroll"><CardContent><FieldGroup>
        <Field><div class="flex items-center justify-between gap-4"><FieldLabel for="autostart">登录时启动 onTime</FieldLabel><Switch id="autostart" :model-value="auto" :disabled="changing" @update:model-value="changeAuto" /></div><FieldDescription>开启后，登录系统时 onTime 会收至托盘并继续监控；手动退出应用后提醒会停止。</FieldDescription></Field>
        <Field><FieldLabel>后台提醒状态</FieldLabel><FieldDescription role="status">{{ reminderRuntimeStatus }}</FieldDescription><FieldDescription>应用需要保持在系统托盘运行；系统睡眠期间不会唤醒电脑。恢复运行后，30 分钟内的漏响会补响，更早的提醒会发送摘要并记录在“我的提醒”页。</FieldDescription></Field>
        <Field><FieldLabel>系统通知</FieldLabel><FieldDescription role="status">{{ store.notificationError || (notificationPermission === 'granted' ? '已获系统授权，提醒触发时会发送桌面通知。' : notificationPermission === 'denied' ? '尚未获系统授权，通知可能不会显示。' : notificationPermission === 'unsupported' ? '当前环境无法读取通知权限。' : '正在检查通知权限…') }} 通知横幅也受系统专注模式或勿扰模式影响；应用响铃窗会作为提醒操作入口。</FieldDescription><div class="flex flex-wrap gap-2"><Button v-if="notificationPermission !== 'granted' && notificationPermission !== 'unsupported'" variant="outline" @click="requestNotificationPermission">请求通知权限</Button><Button variant="outline" @click="testNotification">发送测试通知</Button></div></Field>
        <Field><FieldLabel>提醒铃声</FieldLabel><FieldDescription role="status">{{ store.audioError || '可以试听内置铃声，确认系统音频输出正常。' }}</FieldDescription><div><Button variant="outline" @click="testAudio">测试铃声</Button></div></Field>
      </FieldGroup></CardContent></ScrollArea></Card>
      <Card v-else-if="section === 'data'" class="settings-panel"><CardHeader><CardTitle class="settings-panel-heading">数据</CardTitle><CardDescription>管理本地 SQLite 数据目录。</CardDescription></CardHeader><ScrollArea class="settings-panel-scroll"><CardContent><FieldGroup>
        <Field><FieldLabel>数据状态</FieldLabel><FieldDescription role="status">{{ store.error || (store.snapshot ? 'SQLite 数据库已连接，提醒与分组标签已在本机加载。' : '尚未连接本地数据库。') }}</FieldDescription></Field>
        <Field><FieldLabel>数据目录</FieldLabel><FieldDescription class="break-all">{{ store.snapshot?.dataRoot || '尚未选择' }}</FieldDescription><div><Button variant="outline" :disabled="store.busy" @click="relocate">{{ store.snapshot ? '更改数据目录' : '选择数据目录' }}</Button></div></Field>
      </FieldGroup></CardContent></ScrollArea></Card>
      <Card v-else-if="section === 'backup'" class="settings-panel"><CardHeader><CardTitle class="settings-panel-heading">备份与恢复</CardTitle><CardDescription>导出或恢复本地业务数据。</CardDescription></CardHeader><ScrollArea class="settings-panel-scroll"><CardContent><p class="text-sm text-muted-foreground">onTime 每天首次启动后自动备份 SQLite 数据库，保留最近 7 份，存放在数据目录的 backups 文件夹。手动导出包含提醒、分组、标签、提醒历史和铃声；设置项单独保存在本机。恢复会覆盖当前业务数据，并保留恢复前备份。</p><div class="mt-5 flex flex-wrap gap-2"><Button variant="outline" :disabled="store.busy || !store.snapshot" @click="backup('export_backup')">导出数据备份</Button><Button variant="outline" :disabled="store.busy || !store.snapshot" @click="restoreDialog = true">恢复备份</Button></div></CardContent></ScrollArea></Card>
      <Card v-else class="settings-panel">
        <CardHeader><CardTitle class="settings-panel-heading">关于</CardTitle><CardDescription>版本 {{ version }}</CardDescription></CardHeader>
        <ScrollArea class="settings-panel-scroll"><CardContent><div class="flex flex-col gap-4">
          <p v-if="update?.latest" class="text-sm text-muted-foreground">最新版本 {{ update.latest }}</p>
          <p v-if="update" role="status" class="text-sm">{{ update.message }}</p>
          <div class="flex flex-wrap items-center gap-2">
            <Button variant="outline" :disabled="store.busy" @click="checkUpdate">检查更新</Button>
            <Button v-if="update?.available" variant="outline" :disabled="store.busy || updateProgress === 100" @click="downloadUpdate">{{ downloadingUpdate ? updateProgress === null ? '下载中…' : `下载中 ${updateProgress}%` : updateProgress === 100 ? '下载完成' : '下载安装包' }}</Button>
          </div>
          <p v-if="downloadingUpdate" class="text-xs text-muted-foreground" role="status" aria-live="polite">正在下载更新{{ updateProgress !== null ? `：${updateProgress}%` : '…' }}</p>
          <p v-if="downloadedInstaller" class="break-all text-xs text-muted-foreground" role="status">安装包已保存至：{{ downloadedInstaller }}</p>
          <section v-if="update?.available && update.notes" class="flex flex-col gap-2">
            <h2 class="text-sm font-semibold">更新内容</h2>
            <pre class="whitespace-pre-wrap break-words text-sm text-muted-foreground">{{ update.notes }}</pre>
          </section>
        </div></CardContent></ScrollArea>
      </Card>
    </section>
    <Dialog v-model:open="restoreDialog"><DialogContent><DialogHeader><DialogTitle>覆盖恢复当前数据？</DialogTitle><DialogDescription>恢复会替换提醒、标签和记录，并取消当前活动提醒。应用会保留恢复前备份；取消文件选择不会修改数据。</DialogDescription></DialogHeader><DialogFooter><Button variant="outline" @click="restoreDialog = false">取消</Button><Button variant="destructive" :disabled="store.busy" @click="confirmRestore">选择备份并恢复</Button></DialogFooter></DialogContent></Dialog>
  </main>
</template>







