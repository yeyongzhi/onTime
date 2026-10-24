<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { RouterLink, RouterView, useRoute, onBeforeRouteUpdate } from 'vue-router'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { BellIcon, ListIcon, FolderIcon, TagIcon, HistoryIcon, SettingsIcon, PanelLeftCloseIcon, PanelLeftOpenIcon, SunIcon, MoonIcon, MonitorIcon } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuRadioGroup, DropdownMenuRadioItem } from '@/components/ui/dropdown-menu'
import { Separator } from '@/components/ui/separator'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { useUpdatesStore } from '@/stores/updates'
import { appearance, darkMode } from '@/stores/appearance'
const route = useRoute()
const updates = useUpdatesStore()
onBeforeRouteUpdate(() => !updates.installingUpdate)
let updateTimer: ReturnType<typeof setInterval> | undefined
const collapsed = ref(false)
const closeNotice = ref(false)
const trayHintKey = 'ontime-tray-close-explained-v1'
const closePending = ref(false)
let removeCloseRequest: UnlistenFn | undefined
let disposed = false
onMounted(async () => {
  if (!isTauri()) return
  try {
    const shortcut = localStorage.getItem('ontime.startup-shortcut') || 'Ctrl+Shift+Space'
    if (!await invoke<boolean>('plugin:global-shortcut|is_registered', { shortcut })) {
      await invoke('plugin:global-shortcut|register', { shortcuts: shortcut })
    }
  }
  catch (error) { console.warn('无法注册面板快捷键', error) }
  void updates.showInstallResult()
  void updates.checkUpdate(false)
  updateTimer = setInterval(() => { void updates.checkUpdate(false) }, 6 * 60 * 60 * 1000)
  const unsubscribe = await listen('main-close-request', async () => {
    closePending.value = true
    closeNotice.value = true
  })
  if (disposed) unsubscribe()
  else removeCloseRequest = unsubscribe
})
onUnmounted(() => { disposed = true; removeCloseRequest?.(); if (updateTimer) clearInterval(updateTimer) })
function acknowledgeTrayBehavior() {
  localStorage.setItem(trayHintKey, 'true')
  closeNotice.value = false
  closePending.value = false
}
async function hideToTray() {
  acknowledgeTrayBehavior()
  await getCurrentWindow().hide()
}
function keepWindowOpen() { closeNotice.value = false; closePending.value = false }
const nav = [{ path: '/', name: '我的提醒', icon: BellIcon }, { path: '/reminders', name: '全部提醒', icon: ListIcon }, { path: '/history', name: '提醒历史', icon: HistoryIcon }, { path: '/groups', name: '分组管理', icon: FolderIcon }, { path: '/tags', name: '标签管理', icon: TagIcon }]
function setMode(value: unknown) { if (value === 'light' || value === 'dark' || value === 'system') appearance.mode = value }
const active = (path: string) => route.path === path || (path === '/reminders' && route.path.startsWith('/reminders/'))
</script>
<template>
  <TooltipProvider>
    <div class="app-layout" :data-collapsed="collapsed" :style="{ '--navigation-width': collapsed ? '48px' : `${appearance.sidebarWidth}px` }">
      <aside class="app-sidebar">
        <ScrollArea class="min-h-0 flex-1">
          <nav aria-label="主导航" class="flex flex-col gap-1 p-3">
            <Tooltip v-for="item in nav" :key="item.path">
              <TooltipTrigger as-child><Button variant="ghost" as-child class="navigation-button" :data-active="active(item.path)"><RouterLink :to="item.path" :aria-label="item.name" :aria-current="active(item.path) ? 'page' : undefined"><component :is="item.icon" data-icon="inline-start" /><span v-if="!collapsed">{{ item.name }}</span></RouterLink></Button></TooltipTrigger>
              <TooltipContent v-if="collapsed" side="right">{{ item.name }}</TooltipContent>
            </Tooltip>
          </nav>
        </ScrollArea>
        <Separator v-if="!collapsed" />
        <nav aria-label="应用操作" class="sidebar-actions">
          <Tooltip><TooltipTrigger as-child><Button variant="secondary" size="icon" :aria-label="collapsed ? '展开侧栏' : '收起侧栏'" :aria-expanded="!collapsed" @click="collapsed = !collapsed"><PanelLeftOpenIcon v-if="collapsed" /><PanelLeftCloseIcon v-else /></Button></TooltipTrigger><TooltipContent side="top">{{ collapsed ? '展开侧栏' : '收起侧栏' }}</TooltipContent></Tooltip>
          <DropdownMenu v-if="!collapsed"><DropdownMenuTrigger as-child><Button variant="secondary" size="icon" aria-label="外观模式" title="外观模式"><SunIcon v-if="!darkMode" /><MoonIcon v-else /></Button></DropdownMenuTrigger><DropdownMenuContent side="top" align="end"><DropdownMenuRadioGroup :model-value="appearance.mode" @update:model-value="setMode"><DropdownMenuRadioItem value="light"><SunIcon />亮色</DropdownMenuRadioItem><DropdownMenuRadioItem value="dark"><MoonIcon />暗色</DropdownMenuRadioItem><DropdownMenuRadioItem value="system"><MonitorIcon />跟随系统</DropdownMenuRadioItem></DropdownMenuRadioGroup></DropdownMenuContent></DropdownMenu>
          <Tooltip v-if="!collapsed"><TooltipTrigger as-child><Button variant="secondary" size="icon" as-child :data-active="route.path === '/settings'"><RouterLink to="/settings" aria-label="设置" :aria-current="route.path === '/settings' ? 'page' : undefined"><SettingsIcon /></RouterLink></Button></TooltipTrigger><TooltipContent side="top">设置</TooltipContent></Tooltip>
        </nav>
      </aside>
      <div class="app-content">
        <ScrollArea class="min-h-0 flex-1"><RouterView /></ScrollArea>
      </div>
      <Dialog v-model:open="closeNotice" @update:open="open => { if (!open && closePending) keepWindowOpen() }"><DialogContent><DialogHeader><DialogTitle>onTime 将继续在后台运行</DialogTitle><DialogDescription>关闭窗口会将 onTime 收到系统托盘，已启用的提醒会继续运行。要完全退出，请右键托盘图标并选择“退出（停止提醒）”。</DialogDescription></DialogHeader><DialogFooter><Button variant="secondary" @click="keepWindowOpen">留在窗口</Button><Button @click="hideToTray">收到，最小化到托盘</Button></DialogFooter></DialogContent></Dialog>
    </div>
  </TooltipProvider>
</template>




