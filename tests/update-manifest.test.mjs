import {test} from 'node:test'
import assert from 'node:assert/strict'
import {buildManifest,normalizeRelease,compareTags} from '../scripts/generate-update-manifest.mjs'
function release(tag='v0.0.1-beta5') {
  return {tag_name:tag,prerelease:tag.includes('-'),draft:false,published_at:'2026-10-22T00:00:00Z',body:'说明',assets:['_x64-setup.exe','_x64.dmg','_aarch64.dmg'].map(suffix=>{const name=`onTime_${tag.slice(1)}${suffix}`;return {name,state:'uploaded',size:1024,digest:`sha256:${'a'.repeat(64)}`,browser_download_url:`https://github.com/yeyongzhi/onTime/releases/download/${tag}/${name}`}})}
}
test('发布清单只收录安装包齐全且有校验信息的公开版本',()=>{
  const partial=release('v0.0.1-beta6');partial.assets.pop()
  const draft={...release('v0.0.1-beta7'),draft:true}
  const manifest=buildManifest([partial,draft,release()])
  assert.equal(manifest.preview.tag_name,'v0.0.1-beta5');assert.equal(manifest.stable,null)
  assert.throws(()=>buildManifest([partial,draft]))
})
test('稳定版和预览版独立记录，并按兼容的 beta 数字顺序比较',()=>{
  const manifest=buildManifest([release('v0.0.1-beta2'),release('v0.0.1-beta10'),release('v0.0.1')])
  assert.equal(manifest.preview.tag_name,'v0.0.1-beta10');assert.equal(manifest.stable.tag_name,'v0.0.1')
  assert(compareTags('v0.0.1','v0.0.1-beta10')>0)
})
test('拒绝外部域名、校验信息缺失和错误发布通道',()=>{
  const invalid=release();invalid.assets[0].browser_download_url='https://evil.example/app.exe'
  assert.throws(()=>normalizeRelease(invalid))
  const noHash=release();delete noHash.assets[0].digest;assert.throws(()=>normalizeRelease(noHash))
  assert.throws(()=>normalizeRelease({...release(),prerelease:false}))
})
