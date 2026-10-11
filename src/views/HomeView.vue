<script setup lang="ts">
import { computed, ref } from 'vue'
import { AlarmClockIcon, PlusIcon, Trash2Icon, PencilIcon, TimerIcon, TagIcon, ChevronDownIcon, LayoutGridIcon, ListIcon } from '@lucide/vue'
import type { Alarm, Tag } from '@/lib/types'
import { repeatLabels, dateLabel } from '@/lib/types'
import { useAlarmsStore } from '@/stores/alarms'
import { useRouter } from 'vue-router'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Switch } from '@/components/ui/switch'
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from '@/components/ui/card'
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle, EmptyDescription, EmptyContent } from '@/components/ui/empty'
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { cn } from '@/lib/utils'
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem, DropdownMenuCheckboxItem, DropdownMenuSeparator } from '@/components/ui/dropdown-menu'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { FieldGroup, Field, FieldLabel } from '@/components/ui/field'
const store=useAlarmsStore()
const router=useRouter()
const displayMode=ref('grid')
const managing = ref(false)
const selectedIds = ref<string[]>([])
const batchDelete = ref(false)
const deletingBatch = ref(false)
const query=ref(''); const tagFilter=ref<string[]>([]); const deletion=ref<Alarm | null>(null)
const tagId=ref(''); const tagDeletion=ref<Tag|null>(null); const tagDialog=ref(false); const tagName=ref(''); const tagColor=ref('#7c6cf0')
const filtered=computed(() => store.snapshot?.alarms.filter(a => a.title.toLowerCase().includes(query.value.toLowerCase()) && (tagFilter.value.length === 0 || tagFilter.value.some(id => a.tags.includes(id)))) ?? [])
const selectedVisible = computed(() => filtered.value.filter(a => selectedIds.value.includes(a.id)).map(a => a.id))
function selectAlarm(id: string, checked: boolean | 'indeterminate') { selectedIds.value = checked === true ? [...new Set([...selectedIds.value, id])] : selectedIds.value.filter(value => value !== id) }
function finishManaging() { managing.value = false; selectedIds.value = [] }
async function removeBatch() {
  const ids = [...selectedVisible.value]
  deletingBatch.value = true
  try {
    for (const id of ids) {
      await store.action('delete_alarm', { id })
      selectedIds.value = selectedIds.value.filter(value => value !== id)
    }
    batchDelete.value = false
  } catch { /* Keep remaining selections available to retry. */ }
  finally { deletingBatch.value = false }
}
const next=computed(() => Object.values(store.snapshot?.nextRuns ?? {}).filter(t=>t>=store.now).sort((a,b)=>a-b)[0])
const enabledCount=computed(()=>store.snapshot?.alarms.filter(a=>a.enabled).length ?? 0)
function remaining(a:Alarm) { const t=store.snapshot?.nextRuns[a.id]; if(!t) return a.enabled ? '本次已完成' : '已停用'; if(a.kind==='clock') return dateLabel(t); const d=Math.max(0,t-store.now); return `${Math.floor(d/3600)}:${String(Math.floor(d%3600/60)).padStart(2,'0')}:${String(d%60).padStart(2,'0')}` }
async function toggle(ids:string[],enabled:boolean) { try { await store.action('set_enabled',{ids,enabled},enabled?'已启用':'已停用') } catch { /* toast */ } }
async function chooseRoot() { try { await store.action('choose_data_root') } catch { /* toast */ } }

async function remove() { if(!deletion.value) return; try { await store.action('delete_alarm',{id:deletion.value.id},'提醒已删除'); deletion.value=null } catch { /* toast */ } }
async function saveTag() { try { await store.action('save_tag',{tag:{id:tagId.value,name:tagName.value,color:tagColor.value}},tagId.value?'标签已更新':'标签已创建'); tagName.value=''; tagId.value=''; tagDialog.value=false } catch { /* toast */ } }
function openTag(tag?:Tag) { tagId.value=tag?.id??'';tagName.value=tag?.name??'';tagColor.value=tag?.color??'#7c6cf0';tagDialog.value=true }
function filterTag(id: string, checked: boolean) { tagFilter.value = checked ? [...new Set([...tagFilter.value, id])] : tagFilter.value.filter(value => value !== id) }
async function removeTag() { if(!tagDeletion.value)return;try{await store.action('delete_tag',{id:tagDeletion.value.id},'标签已删除');tagFilter.value = tagFilter.value.filter(id => id !== tagDeletion.value!.id);tagDeletion.value=null}catch{/* toast */} }
</script>
<template>
  <main class="workspace-page">
    <Card class="home-surface"><CardHeader class="flex flex-row items-center justify-between gap-4"><div><p class="text-sm text-muted-foreground">{{ new Date(store.now*1000).toLocaleDateString('zh-CN',{weekday:'long',month:'long',day:'numeric'}) }}</p><CardTitle><h1 class="workspace-heading mt-1">我的提醒</h1></CardTitle></div><Button @click="router.push('/reminders/new')" :disabled="!store.snapshot||!!store.error"><PlusIcon data-icon="inline-start" />新建提醒</Button></CardHeader><CardContent class="flex flex-col gap-6">
    <Alert v-if="store.error" variant="destructive"><AlertTitle>需要处理</AlertTitle><AlertDescription>{{ store.error }}<Button variant="outline" size="sm" class="mt-2" @click="chooseRoot">重新选择数据目录</Button></AlertDescription></Alert>
    <p v-if="store.loading" role="status" class="text-muted-foreground">正在读取本地数据…</p>
    <Empty v-else-if="!store.snapshot"><EmptyHeader><EmptyMedia variant="icon"><AlarmClockIcon /></EmptyMedia><EmptyTitle>欢迎使用 onTime</EmptyTitle><EmptyDescription>选择数据目录，开始创建提醒。</EmptyDescription></EmptyHeader><EmptyContent><Button @click="chooseRoot" :disabled="store.busy">选择数据目录</Button></EmptyContent></Empty>
    <template v-else>
      <div class="reminder-stats"><Card><CardHeader><CardDescription>下一次提醒</CardDescription><CardTitle>{{ next ? dateLabel(next) : '暂无安排' }}</CardTitle></CardHeader></Card><Card><CardHeader><CardDescription>已启用</CardDescription><CardTitle>{{ enabledCount }} <span class="text-sm text-muted-foreground">个提醒</span></CardTitle></CardHeader></Card><Card><CardHeader><CardDescription>全部提醒</CardDescription><CardTitle>{{ store.snapshot.alarms.length }} <span class="text-sm text-muted-foreground">个提醒</span></CardTitle></CardHeader></Card></div>
      <div class="flex flex-wrap items-center gap-3">
        <Input v-model="query" placeholder="搜索提醒名称…" aria-label="搜索提醒" class="max-w-xs" />
        <DropdownMenu><DropdownMenuTrigger as-child><Button variant="outline"><TagIcon data-icon="inline-start" />{{ tagFilter.length ? `标签 · ${tagFilter.length}` : '全部标签' }}<ChevronDownIcon data-icon="inline-end" /></Button></DropdownMenuTrigger><DropdownMenuContent align="start"><DropdownMenuGroup><DropdownMenuCheckboxItem :model-value="tagFilter.length === 0" @update:model-value="checked => { if (checked) tagFilter = [] }" @select.prevent>全部标签</DropdownMenuCheckboxItem><DropdownMenuCheckboxItem v-for="tag in store.snapshot.tags" :key="tag.id" :model-value="tagFilter.includes(tag.id)" @update:model-value="checked => filterTag(tag.id, checked)" @select.prevent><span class="size-2 rounded-full" :style="{ backgroundColor: tag.color }" />{{ tag.name }}</DropdownMenuCheckboxItem></DropdownMenuGroup><DropdownMenuSeparator /><DropdownMenuGroup><DropdownMenuItem @select="openTag()"><TagIcon />新建标签</DropdownMenuItem></DropdownMenuGroup></DropdownMenuContent></DropdownMenu>
        <div class="ml-auto flex items-center gap-3"><Tabs v-model="displayMode"><TabsList aria-label="提醒显示方式"><TabsTrigger value="grid"><LayoutGridIcon />网格</TabsTrigger><TabsTrigger value="list"><ListIcon />列表</TabsTrigger></TabsList></Tabs>
          <Button variant="outline" :disabled="deletingBatch" @click="managing ? finishManaging() : managing = true">{{ managing ? '完成管理' : '批量管理' }}</Button>
        </div>
      </div>
      <div v-if="managing" class="flex flex-wrap items-center gap-3" role="group" aria-label="批量管理">
        <Checkbox id="select-visible" :model-value="selectedVisible.length > 0 && selectedVisible.length === filtered.length ? true : selectedVisible.length ? 'indeterminate' : false" :disabled="deletingBatch || store.busy || !filtered.length" @update:model-value="value => selectedIds = value === true ? filtered.map(a => a.id) : []" /><FieldLabel for="select-visible">全选当前结果</FieldLabel><span class="text-sm text-muted-foreground">已选 {{ selectedVisible.length }} 项</span>
        <Button size="sm" :disabled="!selectedVisible.length || store.busy || deletingBatch" @click="toggle(selectedVisible, true)">启用</Button><Button size="sm" variant="outline" :disabled="!selectedVisible.length || store.busy || deletingBatch" @click="toggle(selectedVisible, false)">停用</Button><Button size="sm" variant="destructive" :disabled="!selectedVisible.length || store.busy || deletingBatch" @click="batchDelete = true">删除</Button>
      </div>
      <div v-if="filtered.length" :class="cn('grid gap-4', displayMode === 'grid' ? 'reminder-grid' : 'reminder-list')"><Card v-for="a in filtered" :key="a.id" :data-selected="managing && selectedIds.includes(a.id)"><CardHeader><div class="flex items-start justify-between gap-3"><div><div class="flex items-center gap-2"><Checkbox v-if="managing" :model-value="selectedIds.includes(a.id)" :aria-label="`选择提醒 ${a.title}`" :disabled="deletingBatch || store.busy" @update:model-value="value => selectAlarm(a.id, value)" /><CardTitle class="font-semibold">{{ a.title }}</CardTitle></div><Badge v-if="displayMode === 'list'" class="mt-2" :variant="a.enabled ? 'default' : 'destructive'">{{ a.enabled ? '已启用' : '已停用' }}</Badge><CardDescription class="mt-2">{{ a.kind==='timer' ? '倒计时' : repeatLabels[a.repeat] }}{{ a.skipHolidays ? ' · 跳过法定假日' : '' }}</CardDescription></div><Switch v-if="displayMode === 'grid'" :model-value="a.enabled" :aria-label="`${a.title}启用开关`" :disabled="store.busy" @update:model-value="v=>toggle([a.id],v)" /></div></CardHeader><CardContent><div v-if="displayMode === 'grid' || a.kind === 'timer'" class="flex items-center gap-3"><component :is="a.kind==='timer'?TimerIcon:AlarmClockIcon" class="size-5 text-muted-foreground" /><span class="text-3xl font-medium tabular-nums tracking-tight">{{ a.kind==='timer'?remaining(a):a.time }}</span></div><p v-if="a.kind==='clock'" class="mt-2 text-sm text-muted-foreground">{{ a.enabled?'下次：':'' }}{{ remaining(a) }}</p><div class="mt-4 flex flex-wrap gap-2"><Badge v-for="tag in store.snapshot.tags.filter(t=>a.tags.includes(t.id))" :key="tag.id" variant="outline"><span class="size-2 rounded-full" :style="{backgroundColor:tag.color}" />{{ tag.name }}</Badge></div></CardContent><CardFooter class="flex justify-between gap-2 px-3 py-2"><Badge v-if="displayMode === 'grid'" :variant="a.enabled?'default':'destructive'">{{ a.enabled?'已启用':'已停用' }}</Badge><div class="flex items-center gap-2"><Switch v-if="displayMode === 'list'" :model-value="a.enabled" :aria-label="`${a.title}启用开关`" :disabled="store.busy" @update:model-value="v => toggle([a.id], v)" /><Tooltip><TooltipTrigger as-child><Button variant="ghost" size="icon-sm" aria-label="编辑提醒" @click="router.push({name:'edit-reminder',params:{id:a.id}})"><PencilIcon /></Button></TooltipTrigger><TooltipContent>编辑提醒</TooltipContent></Tooltip><Tooltip><TooltipTrigger as-child><Button variant="destructive" size="icon-sm" aria-label="删除提醒" @click="deletion=a"><Trash2Icon /></Button></TooltipTrigger><TooltipContent>删除提醒</TooltipContent></Tooltip></div></CardFooter></Card></div>
      <Empty v-else><EmptyHeader><EmptyMedia variant="icon"><AlarmClockIcon /></EmptyMedia><EmptyTitle>{{ store.snapshot.alarms.length?'没有匹配的提醒':'从第一个提醒开始' }}</EmptyTitle><EmptyDescription>点击下方按钮创建提醒。</EmptyDescription></EmptyHeader><EmptyContent><Button variant="outline" @click="router.push('/reminders/new')">新建提醒</Button></EmptyContent></Empty>
    </template>

    </CardContent></Card>
    <Dialog v-model:open="batchDelete"><DialogContent @interact-outside="event => { if (deletingBatch) event.preventDefault() }" @escape-key-down="event => { if (deletingBatch) event.preventDefault() }"><DialogHeader><DialogTitle>删除选中的 {{ selectedVisible.length }} 个提醒？</DialogTitle><DialogDescription>将删除所选提醒并取消它们的活动提醒，历史记录保留。</DialogDescription></DialogHeader><DialogFooter><Button variant="outline" :disabled="deletingBatch" @click="batchDelete = false">取消</Button><Button variant="destructive" :disabled="deletingBatch || store.busy" @click="removeBatch">{{ deletingBatch ? '正在删除…' : '确认删除' }}</Button></DialogFooter></DialogContent></Dialog>
    <Dialog :open="!!deletion" @update:open="v=>{if(!v) deletion=null}"><DialogContent><DialogHeader><DialogTitle>删除提醒？</DialogTitle><DialogDescription>将删除“{{ deletion?.title }}”并取消它的活动提醒，历史记录保留。</DialogDescription></DialogHeader><DialogFooter><Button variant="outline" @click="deletion=null">取消</Button><Button variant="destructive" @click="remove" :disabled="store.busy">删除</Button></DialogFooter></DialogContent></Dialog>
    <Dialog v-model:open="tagDialog"><DialogContent><DialogHeader><DialogTitle>{{ tagId ? '编辑标签' : '新建标签' }}</DialogTitle><DialogDescription>为工作、生活与健康安排不同的颜色。</DialogDescription></DialogHeader><form id="tag-form" @submit.prevent="saveTag"><FieldGroup><Field><FieldLabel for="tag-name">名称</FieldLabel><Input id="tag-name" v-model="tagName" maxlength="24" required /></Field><Field><FieldLabel for="tag-color">颜色</FieldLabel><Input id="tag-color" v-model="tagColor" type="color" /></Field></FieldGroup></form><div v-if="store.snapshot?.tags.length" class="flex flex-col gap-2 border-t pt-4"><p class="text-sm text-muted-foreground">已有标签</p><div v-for="tag in store.snapshot.tags" :key="tag.id" class="flex items-center gap-2"><span class="size-2 rounded-full" :style="{backgroundColor:tag.color}"/><span class="min-w-0 flex-1 truncate">{{ tag.name }}</span><Button variant="ghost" size="icon-sm" :aria-label="`编辑标签${tag.name}`" @click="openTag(tag)"><PencilIcon/></Button><Button variant="ghost" size="icon-sm" :aria-label="`删除标签${tag.name}`" @click="tagDeletion=tag;tagDialog=false"><Trash2Icon/></Button></div></div><DialogFooter><Button variant="outline" @click="tagDialog=false">取消</Button><Button type="submit" form="tag-form" :disabled="store.busy">{{ tagId ? '保存标签' : '创建标签' }}</Button></DialogFooter></DialogContent></Dialog>
<Dialog :open="!!tagDeletion" @update:open="v=>{if(!v)tagDeletion=null}"><DialogContent><DialogHeader><DialogTitle>删除标签？</DialogTitle><DialogDescription>删除“{{ tagDeletion?.name }}”并从提醒上移除此标签。提醒规则保留。</DialogDescription></DialogHeader><DialogFooter><Button variant="outline" @click="tagDeletion=null">取消</Button><Button variant="destructive" :disabled="store.busy" @click="removeTag">删除标签</Button></DialogFooter></DialogContent></Dialog>  </main>
</template>






