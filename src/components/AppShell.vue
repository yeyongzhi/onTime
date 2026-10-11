<script setup lang="ts">
import { ref } from 'vue'
import { RouterLink, RouterView, useRoute } from 'vue-router'
import { BellIcon, HistoryIcon, SettingsIcon, PanelLeftCloseIcon, PanelLeftOpenIcon, SunIcon, MoonIcon, MonitorIcon } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuRadioGroup, DropdownMenuRadioItem } from '@/components/ui/dropdown-menu'
import { Separator } from '@/components/ui/separator'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { appearance, darkMode } from '@/stores/appearance'
const route = useRoute()
const collapsed = ref(false)
const nav = [{ path: '/', name: '我的提醒', icon: BellIcon }, { path: '/history', name: '提醒记录', icon: HistoryIcon }]
function setMode(value: unknown) { if (value === 'light' || value === 'dark' || value === 'system') appearance.mode = value }
const active = (path: string) => route.path === path || (path === '/' && route.path.startsWith('/reminders/'))
</script>
<template>
  <TooltipProvider>
    <div class="app-layout" :data-collapsed="collapsed" :style="{ '--navigation-width': collapsed ? '48px' : `${appearance.sidebarWidth}px` }">
      <aside class="app-sidebar">
        <ScrollArea v-if="!collapsed" class="min-h-0 flex-1">
          <nav aria-label="主导航" class="flex flex-col gap-1 p-3">
            <Tooltip v-for="item in nav" :key="item.path">
              <TooltipTrigger as-child><Button variant="ghost" as-child class="navigation-button" :data-active="active(item.path)"><RouterLink :to="item.path" :aria-label="item.name" :aria-current="active(item.path) ? 'page' : undefined"><component :is="item.icon" data-icon="inline-start" /><span v-if="!collapsed">{{ item.name }}</span></RouterLink></Button></TooltipTrigger>
              <TooltipContent v-if="collapsed" side="right">{{ item.name }}</TooltipContent>
            </Tooltip>
          </nav>
        </ScrollArea>
        <Separator v-if="!collapsed" />
        <nav aria-label="应用操作" class="sidebar-actions">
          <Tooltip><TooltipTrigger as-child><Button variant="ghost" size="icon" :aria-label="collapsed ? '展开侧栏' : '收起侧栏'" :aria-expanded="!collapsed" @click="collapsed = !collapsed"><PanelLeftOpenIcon v-if="collapsed" /><PanelLeftCloseIcon v-else /></Button></TooltipTrigger><TooltipContent side="top">{{ collapsed ? '展开侧栏' : '收起侧栏' }}</TooltipContent></Tooltip>
          <Tooltip v-if="!collapsed"><TooltipTrigger as-child><Button variant="ghost" size="icon" as-child :data-active="route.path === '/settings'"><RouterLink to="/settings" aria-label="设置" :aria-current="route.path === '/settings' ? 'page' : undefined"><SettingsIcon /></RouterLink></Button></TooltipTrigger><TooltipContent side="top">设置</TooltipContent></Tooltip>
          <DropdownMenu v-if="!collapsed"><DropdownMenuTrigger as-child><Button variant="ghost" size="icon" aria-label="外观模式" title="外观模式"><SunIcon v-if="!darkMode" /><MoonIcon v-else /></Button></DropdownMenuTrigger><DropdownMenuContent side="top" align="end"><DropdownMenuRadioGroup :model-value="appearance.mode" @update:model-value="setMode"><DropdownMenuRadioItem value="light"><SunIcon />亮色</DropdownMenuRadioItem><DropdownMenuRadioItem value="dark"><MoonIcon />暗色</DropdownMenuRadioItem><DropdownMenuRadioItem value="system"><MonitorIcon />跟随系统</DropdownMenuRadioItem></DropdownMenuRadioGroup></DropdownMenuContent></DropdownMenu>
        </nav>
      </aside>
      <div class="app-content">
        <ScrollArea class="min-h-0 flex-1"><RouterView /></ScrollArea>
      </div>
    </div>
  </TooltipProvider>
</template>




