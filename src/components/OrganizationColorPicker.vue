<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { CheckIcon, PaletteIcon } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Field, FieldLabel } from '@/components/ui/field'
import { Popover, PopoverContent, PopoverTrigger, PopoverHeader, PopoverTitle } from '@/components/ui/popover'
import { cn } from '@/lib/utils'

const color = defineModel<string>({ required: true })
defineProps<{ disabled?: boolean }>()
const presets = [
  { name: '紫色', value: '#7c6cf0' },
  { name: '蓝色', value: '#3b82f6' },
  { name: '青色', value: '#06b6d4' },
  { name: '绿色', value: '#22c55e' },
  { name: '黄色', value: '#eab308' },
  { name: '橙色', value: '#f97316' },
  { name: '红色', value: '#ef4444' },
  { name: '粉色', value: '#ec4899' },
]
const hsv = computed(() => {
  const hex = color.value.replace('#', '')
  const [r, g, b] = [0, 2, 4].map(i => parseInt(hex.slice(i, i + 2), 16) / 255) as [number, number, number]
  const max = Math.max(r, g, b), min = Math.min(r, g, b), delta = max - min
  let hue = delta === 0 ? 0 : max === r ? ((g - b) / delta) % 6 : max === g ? (b - r) / delta + 2 : (r - g) / delta + 4
  hue = (hue * 60 + 360) % 360
  return { h: hue, s: max === 0 ? 0 : delta / max, v: max }
})
function setHsv(h: number, s: number, v: number) {
  const channel = (n: number) => {
    const k = (n + h / 60) % 6
    return Math.round(255 * v * (1 - s * Math.max(0, Math.min(k, 4 - k, 1)))).toString(16).padStart(2, '0')
  }
  color.value = `#${channel(5)}${channel(3)}${channel(1)}`
}
const marker = computed(() => {
  const angle = hsv.value.h * Math.PI / 180
  return { left: `${50 + Math.sin(angle) * hsv.value.s * 50}%`, top: `${50 - Math.cos(angle) * hsv.value.s * 50}%` }
})
function selectOnWheel(event: PointerEvent) {
  const element = event.currentTarget as HTMLElement
  if (event.type === 'pointermove' && !element.hasPointerCapture(event.pointerId)) return
  if (event.type === 'pointerdown') element.setPointerCapture(event.pointerId)
  const bounds = element.getBoundingClientRect()
  const x = (event.clientX - bounds.left) / bounds.width * 2 - 1
  const y = (event.clientY - bounds.top) / bounds.height * 2 - 1
  const hue = (Math.atan2(x, -y) * 180 / Math.PI + 360) % 360
  setHsv(hue, Math.min(1, Math.hypot(x, y)), hsv.value.v || 1)
}
function wheelKey(event: KeyboardEvent) {
  const { h, s, v } = hsv.value
  if (['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) {
    event.preventDefault()
    setHsv((h + (event.key === 'ArrowRight' ? 3 : event.key === 'ArrowLeft' ? -3 : 0) + 360) % 360,
      Math.max(0, Math.min(1, s + (event.key === 'ArrowUp' ? 0.02 : event.key === 'ArrowDown' ? -0.02 : 0))), v || 1)
  }
}
const hexDraft = ref(color.value)
watch(color, value => { hexDraft.value = value })
function setHex(value: string | number) {
  hexDraft.value = String(value)
  if (/^#[0-9a-f]{6}$/i.test(hexDraft.value)) color.value = hexDraft.value.toLowerCase()
}
const custom = computed(() => !presets.some(p => p.value === color.value.toLowerCase()))
</script>
<template>
  <div class="flex flex-nowrap items-center gap-1" role="group" aria-label="颜色选择">
    <Button v-for="preset in presets" :key="preset.value" type="button" variant="outline" size="icon"
      :class="cn('size-7 rounded-full', color.toLowerCase() === preset.value && 'ring-2 ring-ring ring-offset-2 ring-offset-background')"
      :disabled="disabled" :aria-label="preset.name" :title="preset.name" :aria-pressed="color.toLowerCase() === preset.value"
      @click="color = preset.value">
      <span class="flex size-4 items-center justify-center rounded-full" :style="{ backgroundColor: preset.value }">
        <CheckIcon v-if="color.toLowerCase() === preset.value" class="text-white" />
      </span>
    </Button>
    <Popover>
      <PopoverTrigger as-child><Button type="button" variant="secondary" size="sm" :disabled="disabled" :class="cn(custom && 'ring-2 ring-ring')" aria-label="选择自定义颜色">
        <span v-if="custom" class="size-4 rounded-full" :style="{ backgroundColor: color }" /><PaletteIcon v-else data-icon="inline-start" />自定义
      </Button></PopoverTrigger>
      <PopoverContent align="end"><PopoverHeader><PopoverTitle>自定义颜色</PopoverTitle></PopoverHeader>
        <div class="color-wheel" role="group" tabindex="0" aria-label="圆形颜色色盘，左右方向键调整色相，上下方向键调整饱和度" @pointerdown.prevent="selectOnWheel" @pointermove="selectOnWheel" @pointerup="($event.currentTarget as HTMLElement).releasePointerCapture($event.pointerId)" @keydown="wheelKey">
          <span class="color-wheel-shade" :style="{ opacity: 1 - hsv.v }" /><span class="color-wheel-marker" :style="marker" />
        </div>
        
        <Field><FieldLabel for="organization-custom-color">十六进制颜色</FieldLabel><Input id="organization-custom-color" :model-value="hexDraft" maxlength="7" placeholder="#7c6cf0" :disabled="disabled" @update:model-value="setHex" @blur="hexDraft = color" /></Field>
        <p class="text-sm text-muted-foreground">当前颜色：{{ color.toUpperCase() }}</p>
      </PopoverContent>
    </Popover>
  </div>
</template>

<style scoped>
.color-wheel { position: relative; width: 180px; max-width: 100%; margin: 8px auto; aspect-ratio: 1; border-radius: 50%; touch-action: none; cursor: crosshair; background: radial-gradient(circle closest-side, white, transparent), conic-gradient(red, yellow, lime, cyan, blue, magenta, red); }
.color-wheel:focus-visible { outline: 2px solid var(--ring); outline-offset: 4px; }
.color-wheel-shade { position: absolute; inset: 0; border-radius: inherit; background: black; pointer-events: none; }
.color-wheel-marker { position: absolute; width: 16px; height: 16px; border: 2px solid white; border-radius: 50%; transform: translate(-50%, -50%); box-shadow: 0 0 2px 1px #0006; pointer-events: none; }
</style>
