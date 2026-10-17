import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { toast } from 'vue-sonner'
import { computed, reactive, watch } from 'vue'

type Mode = 'light' | 'dark' | 'system'
interface Appearance { mode: Mode; color: string; fontSize: number; radius: number; sidebarWidth: number }
const key = 'ontime.appearance.v1'
const defaults: Appearance = { mode: 'system', color: '#00b954', fontSize: 14, radius: 16, sidebarWidth: 140 }
function normalize(value: Partial<Appearance>): Appearance {
  const number = (v: unknown, fallback: number, min: number, max: number) => typeof v === 'number' && Number.isFinite(v) ? Math.round(Math.min(max, Math.max(min, v))) : fallback
  return {
    mode: ['light', 'dark', 'system'].includes(value.mode ?? '') ? value.mode! : defaults.mode,
    color: typeof value.color === 'string' && /^#[\da-f]{6}$/i.test(value.color) ? value.color : defaults.color,
    fontSize: number(value.fontSize, defaults.fontSize, 12, 24),
    radius: number(value.radius, defaults.radius, 0, 24),
    sidebarWidth: number(value.sidebarWidth, defaults.sidebarWidth, 120, 320),
  }
}
function read(): Appearance {
  try { return normalize(JSON.parse(localStorage.getItem(key) ?? '{}') ?? {}) } catch { return { ...defaults } }
}
export const appearance = reactive(read())
const media = window.matchMedia('(prefers-color-scheme: dark)')
const systemDark = reactive({ value: media.matches })
export const darkMode = computed(() => appearance.mode === 'dark' || (appearance.mode === 'system' && systemDark.value))
let syncing = false
let nativeReady = false
let saveQueue = Promise.resolve()
function apply() {
  const root = document.documentElement
  const dark = appearance.mode === 'dark' || (appearance.mode === 'system' && media.matches)
  root.classList.toggle('dark', dark)
  root.style.colorScheme = dark ? 'dark' : 'light'
  root.style.fontSize = `${appearance.fontSize}px`
  root.style.setProperty('--radius', `${appearance.radius / appearance.fontSize}rem`)
  root.style.setProperty('--app-sidebar-width', `${appearance.sidebarWidth}px`)
  const rgb = appearance.color.slice(1).match(/../g)!.map(v => parseInt(v, 16) / 255)
  const luminance = (channels: number[]) => channels.map(v => v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4).reduce((sum, v, i) => sum + v * [0.2126, 0.7152, 0.0722][i]!, 0)
  let buttonRgb = [...rgb]
  while (luminance(buttonRgb) > 0.18) buttonRgb = buttonRgb.map(v => v * 0.95)
  const primary = '#' + buttonRgb.map(v => Math.round(v * 255).toString(16).padStart(2, '0')).join('')
  for (const name of ['primary', 'ring', 'sidebar-primary', 'sidebar-ring']) root.style.setProperty(`--${name}`, primary)
  for (const name of ['primary-foreground', 'sidebar-primary-foreground']) root.style.setProperty(`--${name}`, '#ffffff')
  root.style.setProperty('--theme-soft', `color-mix(in srgb, ${appearance.color} ${dark ? 14 : 6}%, var(--card))`)
  root.style.setProperty('--theme-ink', `color-mix(in srgb, ${appearance.color} ${dark ? 65 : 75}%, ${dark ? 'white' : 'black'})`)
}
watch(appearance, () => {
  apply()
  if (!syncing) {
    if (isTauri() && nativeReady) {
      const value = { ...appearance }
      saveQueue = saveQueue.then(() => invoke('save_appearance', { value })).then(() => {}).catch(e => { toast.error(String(e)) })
    }
    try { localStorage.setItem(key, JSON.stringify(appearance)) } catch { /* Keep the current session usable when storage is unavailable. */ }
  }
}, { deep: true, flush: 'sync' })
media.addEventListener('change', () => { systemDark.value = media.matches; apply() })
window.addEventListener('storage', event => {
  if (event.key !== key && event.key !== null) return
  syncing = true
  try { Object.assign(appearance, read()) } finally { syncing = false }
})
apply()
export function resetAppearance() { Object.assign(appearance, defaults) }


export async function loadAppearance() {
  if (!isTauri()) return
  try {
    const stored = await invoke<Partial<Appearance> | null>('read_appearance')
    syncing = true
    if (stored) Object.assign(appearance, normalize(stored))
    syncing = false
    nativeReady = true
    if (!stored) await invoke('save_appearance', { value: { ...appearance } })
  } catch (e) { syncing = false; nativeReady = false; toast.error(String(e)) }
}
if (isTauri()) {
  void listen<Appearance>('appearance-changed', event => {
    syncing = true
    try { Object.assign(appearance, normalize(event.payload)) } finally { syncing = false }
  })
  void loadAppearance()
}
