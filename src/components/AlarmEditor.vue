<script setup lang="ts">
import { computed, reactive, ref, onUnmounted } from 'vue'
import { Music2Icon, PlayIcon, SquareIcon, ArrowLeftIcon } from '@lucide/vue'
import type { Alarm } from '@/lib/types'
import { invoke } from '@tauri-apps/api/core'
import { repeatLabels } from '@/lib/types'
import { useAlarmsStore } from '@/stores/alarms'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { FieldGroup, Field, FieldLabel, FieldDescription, FieldSet, FieldLegend } from '@/components/ui/field'
import { Select, SelectTrigger, SelectValue, SelectContent, SelectGroup, SelectItem } from '@/components/ui/select'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Slider } from '@/components/ui/slider'
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert'
const props = defineProps<{ alarm: Alarm }>()
const emit = defineEmits<{ close: [] }>()
const store = useAlarmsStore()
const form = reactive<Alarm>(JSON.parse(JSON.stringify(props.alarm)))
const minutes = ref(form.durationSeconds / 60)
const selectedDays = ref(form.weekdays.map(String))
const kind = computed({ get: () => form.kind, set: v => { if(v) form.kind = v } })
const volume = computed({ get: () => [Math.round(form.volume * 100)], set: (value: number[]) => { if(value[0] !== undefined) form.volume = value[0] / 100 } })
const previewing = ref(false)
let previewTimer: ReturnType<typeof setTimeout> | undefined
onUnmounted(() => { if(previewTimer)clearTimeout(previewTimer); if(previewing.value)void invoke('preview_sound',{sound:null,volume:0,stop:true}).catch(()=>{}) })
const formError = ref('')
async function importSound() { try { const sound = await store.action<string | null>('import_sound'); if(sound) form.sound = sound } catch { /* toast reports */ } }
async function preview() { try { await store.action('preview_sound', {sound:form.sound,volume:Number(form.volume),stop:previewing.value}); previewing.value=!previewing.value; if(previewTimer)clearTimeout(previewTimer); if(previewing.value)previewTimer=setTimeout(()=>{previewing.value=false},10000) } catch { /* toast reports */ } }
async function close() { if(store.busy)return; if(previewing.value) { try { await store.action('preview_sound',{sound:null,volume:0,stop:true}) } catch { /* toast */ } } emit('close') }
async function save() {
  formError.value=''
  if(!form.title.trim()) { formError.value='请填写提醒名称'; return }
  form.durationSeconds=Math.round(Number(minutes.value)*60); form.weekdays=selectedDays.value.map(Number)
  form.volume=Number(form.volume); form.ringSeconds=Number(form.ringSeconds); form.snoozeLimit=Number(form.snoozeLimit)
  try { await store.action('save_alarm',{alarm:form},'提醒已保存'); await close() } catch(e) { formError.value=String(e) }
}
</script>
<template>
  <main class="editor-page">
    <header class="editor-heading">
      <div class="flex items-center gap-3"><Button variant="ghost" size="icon" aria-label="返回提醒列表" :disabled="store.busy" @click="close"><ArrowLeftIcon /></Button><h1 class="workspace-heading">{{ alarm.id ? '编辑提醒' : '新建提醒' }}</h1></div>
    </header><Card class="editor-surface"><ScrollArea class="min-h-0 flex-1"><CardContent class="flex flex-col gap-5">
    <Alert v-if="formError" variant="destructive"><AlertTitle>无法保存提醒</AlertTitle><AlertDescription>{{ formError }}</AlertDescription></Alert>
    <form id="alarm-form" @submit.prevent="save" class="flex flex-col gap-5">
      <Card><CardHeader><CardTitle>时间与规则</CardTitle><CardDescription>设置提醒的名称、时间和重复方式。</CardDescription></CardHeader><CardContent><FieldSet :disabled="store.busy"><FieldGroup>
        <Field :data-invalid="!!formError"><FieldLabel for="alarm-title">名称</FieldLabel><Input id="alarm-title" v-model="form.title" placeholder="例如：午休结束、吃药、团队会议" maxlength="80" :aria-invalid="!!formError" required autofocus /></Field>
        <Field><FieldLabel id="reminder-kind-label">提醒方式</FieldLabel><ToggleGroup v-model="kind" type="single" variant="outline" :spacing="2" aria-labelledby="reminder-kind-label"><ToggleGroupItem value="clock">定点提醒</ToggleGroupItem><ToggleGroupItem value="timer">倒计时</ToggleGroupItem></ToggleGroup></Field>
        <Field v-if="form.kind === 'timer'"><FieldLabel for="minutes">多少分钟后提醒</FieldLabel><Input id="minutes" v-model="minutes" type="number" min="0.02" max="10080" step="0.01" required /><FieldDescription>保存或重新启用时开始计时，包含睡眠时间。</FieldDescription></Field>
        <template v-else>
          <Field><FieldLabel for="alarm-time">提醒时间</FieldLabel><Input id="alarm-time" v-model="form.time" type="time" required /></Field>
          <Field><FieldLabel for="alarm-repeat">重复</FieldLabel><Select v-model="form.repeat"><SelectTrigger id="alarm-repeat" class="w-full"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem v-for="(label,key) in repeatLabels" :key="key" :value="key">{{ label }}</SelectItem></SelectGroup></SelectContent></Select></Field>
          <Field v-if="form.repeat === 'once'"><FieldLabel for="alarm-date">日期</FieldLabel><Input id="alarm-date" :model-value="form.date ?? ''" @update:model-value="v => form.date=String(v)" type="date" required /></Field>
          <FieldSet v-if="form.repeat === 'custom'"><FieldLegend>重复星期</FieldLegend><ToggleGroup v-model="selectedDays" type="multiple" variant="outline" :spacing="1" class="flex-wrap" aria-label="重复星期"><ToggleGroupItem v-for="(day,i) in ['一','二','三','四','五','六','日']" :key="i" :value="String(i)">周{{ day }}</ToggleGroupItem></ToggleGroup></FieldSet>
          <Field v-if="form.repeat !== 'once'" orientation="horizontal"><FieldLabel for="skip-holidays">跳过法定假日</FieldLabel><Switch id="skip-holidays" v-model="form.skipHolidays" /></Field>
        </template>
        <Field orientation="horizontal"><FieldLabel for="alarm-enabled">保存后启用</FieldLabel><Switch id="alarm-enabled" v-model="form.enabled" /></Field>
        <Field v-if="store.snapshot?.tags.length"><FieldLabel id="reminder-tags-label">标签</FieldLabel><ToggleGroup v-model="form.tags" type="multiple" variant="outline" :spacing="2" class="flex-wrap" aria-labelledby="reminder-tags-label"><ToggleGroupItem v-for="tag in store.snapshot.tags" :key="tag.id" :value="tag.id">{{ tag.name }}</ToggleGroupItem></ToggleGroup></Field>
      </FieldGroup></FieldSet></CardContent></Card>
      <Card><CardHeader><CardTitle>响铃与提醒</CardTitle><CardDescription>设置铃声、音量和贪睡次数。</CardDescription></CardHeader><CardContent><FieldSet :disabled="store.busy"><FieldGroup>
        <Field><FieldLabel>铃声</FieldLabel><div class="flex flex-wrap items-center gap-2"><Button type="button" variant="outline" :disabled="store.busy" @click="importSound"><Music2Icon data-icon="inline-start" />{{ form.sound ? '更换音频' : '导入音频' }}</Button><Button type="button" variant="ghost" :disabled="store.busy" @click="form.sound=null">内置轻铃</Button><Button type="button" variant="outline" :disabled="store.busy" @click="preview"><component :is="previewing ? SquareIcon : PlayIcon" data-icon="inline-start" />{{ previewing ? '停止试听' : '试听' }}</Button></div><FieldDescription>{{ form.sound ? '已选择本地音频' : '使用内置轻铃' }}</FieldDescription></Field>
        <Field><FieldLabel id="volume-label">音量 {{ volume[0] }}%</FieldLabel><Slider id="alarm-volume" v-model="volume" :min="0" :max="100" :step="5" :disabled="store.busy" aria-labelledby="volume-label" /></Field>
        <Field orientation="horizontal"><FieldLabel for="alarm-gradual">音量渐强</FieldLabel><Switch id="alarm-gradual" v-model="form.gradual" /></Field>
        <Field><FieldLabel for="alarm-duration">自动停止（秒）</FieldLabel><Input id="alarm-duration" v-model="form.ringSeconds" type="number" min="1" max="3600" required /></Field>
        <Field><FieldLabel for="alarm-snooze">最多贪睡次数</FieldLabel><Input id="alarm-snooze" v-model="form.snoozeLimit" type="number" min="0" max="10" required /></Field>
      </FieldGroup></FieldSet></CardContent></Card>
    </form>
    </CardContent></ScrollArea>
    <CardFooter class="editor-footer bg-card"><Button variant="outline" :disabled="store.busy" @click="close">取消</Button><Button form="alarm-form" type="submit" :disabled="store.busy">{{ store.busy ? '正在保存…' : '保存提醒' }}</Button></CardFooter>
    </Card>
  </main>
</template>



