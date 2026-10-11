<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getVersion } from '@tauri-apps/api/app'
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart'
import { PaletteIcon, SlidersHorizontalIcon, DatabaseIcon, ArchiveIcon, InfoIcon, SunIcon, MoonIcon, MonitorIcon } from '@lucide/vue'
import { toast } from 'vue-sonner'
import { useAlarmsStore } from '@/stores/alarms'
import { appearance, resetAppearance } from '@/stores/appearance'
import { Button } from '@/components/ui/button'
import { Select, SelectTrigger, SelectValue, SelectContent, SelectGroup, SelectItem } from '@/components/ui/select'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card'
import { FieldGroup, Field, FieldLabel, FieldDescription } from '@/components/ui/field'

import { Questionnaire, QuestionnaireItem, QuestionnaireTitle, QuestionnaireChoices, QuestionnaireChoice } from '@/components/ui/questionnaire'

import { ScrollArea } from '@/components/ui/scroll-area'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
const sections = [
  { id: 'appearance', label: '外观', icon: PaletteIcon },
  { id: 'general', label: '常规', icon: SlidersHorizontalIcon },
  { id: 'data', label: '数据', icon: DatabaseIcon },
  { id: 'backup', label: '备份', icon: ArchiveIcon },
  { id: 'about', label: '关于', icon: InfoIcon },
]
const section = ref('appearance')
const store = useAlarmsStore()
const restoreDialog = ref(false)
const auto = ref(false)
const changing = ref(false)
const version = ref('0.1.0')
const colors = [{ value: '#f43f5e', label: '红' }, { value: '#f97316', label: '橙' }, { value: '#eab308', label: '黄' }, { value: '#00b954', label: '绿' }, { value: '#3b82f6', label: '蓝' }, { value: '#6366f1', label: '靛' }, { value: '#a855f7', label: '紫' }]
const customColorDialog = ref(false)
const customColorSelected = ref(!colors.some(item => item.value === appearance.color))
const modes = [{ value: 'light', label: '亮色', icon: SunIcon }, { value: 'dark', label: '暗色', icon: MoonIcon }, { value: 'system', label: '跟随系统', icon: MonitorIcon }]
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
onMounted(async () => {
  try { auto.value = await isEnabled() } catch { /* Native settings are unavailable in browser previews. */ }
  try { version.value = await getVersion() } catch { /* Keep the package version in browser previews. */ }
})
function mode(value: unknown) { if (value === 'light' || value === 'dark' || value === 'system') appearance.mode = value }

async function changeAuto(value: boolean) {
  changing.value = true
  try { if (value) await enable(); else await disable(); auto.value = await isEnabled(); toast.success('自启动设置已更新') }
  catch (e) { toast.error(String(e)) } finally { changing.value = false }
}
async function updateCalendar() { try { const message = await store.action<string>('refresh_calendar', { year: new Date().getFullYear() }); if (message) toast.success(message) } catch { /* Store shows errors. */ } }
async function backup(command: string) { try { const message = await store.action<string | null>(command); if (message) toast.success(message) } catch { /* Store shows errors. */ } }
async function confirmRestore() { restoreDialog.value = false; await backup('import_backup') }
async function relocate() { try { await store.action('choose_data_root') } catch { /* Store shows errors. */ } }
async function checkUpdate() { try { const result = await store.action<UpdateInfo>('check_update'); if (result) update.value = result } catch { /* Store shows errors. */ } }
async function downloadUpdate() { if (update.value?.url) { try { await store.action('open_update_link', { url: update.value.url }) } catch { /* Store shows errors. */ } } }
</script>
<template>
  <main class="settings-layout">
    <nav aria-label="设置分类" class="settings-nav">
      <Card size="sm"><CardHeader><CardTitle><h1>设置</h1></CardTitle></CardHeader><CardContent><ScrollArea class="settings-menu-scroll"><div class="flex flex-col gap-1">
        <Button v-for="item in sections" :key="item.id" variant="ghost" class="navigation-button" :data-active="section === item.id" :aria-current="section === item.id ? 'page' : undefined" @click="section = item.id"><component :is="item.icon" data-icon="inline-start" /><span>{{ item.label }}</span></Button>
      </div></ScrollArea></CardContent></Card>
    </nav>
    <section class="settings-content" :aria-label="sections.find(item => item.id === section)?.label">
      <Card v-if="section === 'appearance'" class="settings-panel">
        <CardHeader class="flex flex-row flex-wrap items-center gap-3"><Badge variant="secondary" class="size-10 shrink-0 justify-center"><PaletteIcon /></Badge><div><CardTitle>外观</CardTitle><CardDescription>调整应用的颜色与显示方式。</CardDescription></div><Button variant="ghost" size="sm" class="ml-auto" @click="resetAppearance(); customColorSelected = false">恢复默认</Button></CardHeader>
        <CardContent><FieldGroup class="appearance-fields">
          <Questionnaire :items="[{ name: 'appearance-mode', required: true }]" @submit.prevent><QuestionnaireItem name="appearance-mode" required><QuestionnaireTitle>外观模式</QuestionnaireTitle><QuestionnaireChoices class="grid grid-cols-3 gap-2"><QuestionnaireChoice v-for="item in modes" :key="item.value" :value="item.value" :checked="appearance.mode === item.value" @update:checked="checked => { if (checked) mode(item.value) }"><span class="flex min-w-0 items-center justify-end gap-2"><component :is="item.icon" class="size-4 shrink-0" /><span>{{ item.label }}</span></span></QuestionnaireChoice></QuestionnaireChoices></QuestionnaireItem></Questionnaire>
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
          <Field><FieldLabel for="sidebar-width">侧边栏宽度</FieldLabel><Select :model-value="String(appearance.sidebarWidth)" @update:model-value="preference($event,'sidebarWidth')"><SelectTrigger id="sidebar-width" class="preference-select"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem v-for="value in widths" :key="value" :value="String(value)">{{ value }} px</SelectItem></SelectGroup></SelectContent></Select></Field>
          <Field><FieldLabel for="corner-radius">全局圆角</FieldLabel><Select :model-value="String(appearance.radius)" @update:model-value="preference($event,'radius')"><SelectTrigger id="corner-radius" class="preference-select"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem v-for="value in radii" :key="value" :value="String(value)">{{ radiusLabel(value) }}</SelectItem></SelectGroup></SelectContent></Select></Field>
          <Field><FieldLabel for="font-size">全局字体大小</FieldLabel><Select :model-value="String(appearance.fontSize)" @update:model-value="preference($event,'fontSize')"><SelectTrigger id="font-size" class="preference-select"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem v-for="value in fonts" :key="value" :value="String(value)">{{ value }} px{{ value === 14 ? '（默认）' : '' }}</SelectItem></SelectGroup></SelectContent></Select></Field>
        </FieldGroup></CardContent>
      </Card>
      <Card v-else-if="section === 'general'" class="settings-panel"><CardHeader class="flex flex-row flex-wrap items-center gap-3"><Badge variant="secondary" class="size-10 shrink-0 justify-center"><SlidersHorizontalIcon /></Badge><div><CardTitle>常规</CardTitle><CardDescription>管理应用启动方式。</CardDescription></div></CardHeader><CardContent><FieldGroup><Field orientation="horizontal"><FieldLabel for="autostart">登录时启动 onTime</FieldLabel><Switch id="autostart" :model-value="auto" :disabled="changing" @update:model-value="changeAuto" /></Field></FieldGroup></CardContent></Card>
      <Card v-else-if="section === 'data'" class="settings-panel"><CardHeader class="flex flex-row flex-wrap items-center gap-3"><Badge variant="secondary" class="size-10 shrink-0 justify-center"><DatabaseIcon /></Badge><div><CardTitle>数据</CardTitle><CardDescription>管理数据目录与节假日日历。</CardDescription></div></CardHeader><CardContent><FieldGroup>
        <Field><FieldLabel>数据目录</FieldLabel><FieldDescription class="break-all">{{ store.snapshot?.dataRoot || '尚未选择' }}</FieldDescription><div><Button variant="outline" :disabled="store.busy" @click="relocate">选择数据目录</Button></div></Field>
        <Field><FieldLabel>中国大陆节假日日历</FieldLabel><FieldDescription>已缓存：{{ store.snapshot?.calendars.map(c => c.year).join('、') || '暂无' }}</FieldDescription><p v-if="!store.snapshot?.calendars.find(c => c.year === new Date().getFullYear())" class="text-sm text-destructive">当年日历缺失，暂按普通星期规则提醒。</p><div><Button variant="outline" :disabled="store.busy || !store.snapshot" @click="updateCalendar">更新当年日历</Button></div></Field>
      </FieldGroup></CardContent></Card>
      <Card v-else-if="section === 'backup'" class="settings-panel"><CardHeader class="flex flex-row flex-wrap items-center gap-3"><Badge variant="secondary" class="size-10 shrink-0 justify-center"><ArchiveIcon /></Badge><div><CardTitle>备份与恢复</CardTitle><CardDescription>保存或恢复完整的提醒数据。</CardDescription></div></CardHeader><CardContent><p class="text-sm text-muted-foreground">包含提醒、标签、记录、日历和铃声。恢复将覆盖当前数据，并保留恢复前备份。</p><div class="mt-5 flex flex-wrap gap-2"><Button variant="outline" :disabled="store.busy || !store.snapshot" @click="backup('export_backup')">导出备份</Button><Button variant="outline" :disabled="store.busy || !store.snapshot" @click="restoreDialog = true">恢复备份</Button></div></CardContent></Card>
      <Card v-else class="settings-panel"><CardHeader class="flex flex-row flex-wrap items-center gap-3"><Badge variant="secondary" class="size-10 shrink-0 justify-center"><InfoIcon /></Badge><div><CardTitle>关于 onTime</CardTitle><CardDescription>版本 {{ version }}</CardDescription></div></CardHeader><CardContent><div class="flex flex-col gap-4"><p v-if="update?.latest" class="text-sm text-muted-foreground">最新版本 {{ update.latest }}</p><p v-if="update" role="status" class="text-sm">{{ update.message }}</p><pre v-if="update?.notes" class="whitespace-pre-wrap break-words text-sm text-muted-foreground">{{ update.notes }}</pre><div class="flex flex-wrap gap-2"><Button variant="outline" :disabled="store.busy" @click="checkUpdate">检查更新</Button><Button v-if="update?.available" :disabled="store.busy" @click="downloadUpdate">下载安装包</Button></div></div></CardContent></Card>
    </section>
    <Dialog v-model:open="restoreDialog"><DialogContent><DialogHeader><DialogTitle>覆盖恢复当前数据？</DialogTitle><DialogDescription>恢复会替换提醒、标签和记录，并取消当前活动提醒。应用会保留恢复前备份；取消文件选择不会修改数据。</DialogDescription></DialogHeader><DialogFooter><Button variant="outline" @click="restoreDialog = false">取消</Button><Button variant="destructive" :disabled="store.busy" @click="confirmRestore">选择备份并恢复</Button></DialogFooter></DialogContent></Dialog>
  </main>
</template>







