import { readFileSync, writeFileSync, mkdirSync } from 'node:fs'
import { dirname } from 'node:path'
import { pathToFileURL } from 'node:url'
const prefix = 'https://github.com/yeyongzhi/onTime/releases/download/'
const platforms = ['_x64-setup.exe', '_aarch64.dmg', '_x64.dmg']
function version(tag) {
  const value = tag.replace(/^v/, '')
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.test(value)) throw new Error('Invalid release version')
  const core=value.split('+')[0], separator=core.indexOf('-')
  const base=separator<0?core:core.slice(0,separator), pre=separator<0?'':core.slice(separator+1)
  const prerelease = pre.replace(/^([a-z]+)(\d+)$/i, (_, label, number) => `${label}.${BigInt(number)}`)
  return { numbers: base.split('.').map(BigInt), pre: prerelease ? prerelease.split('.') : [] }
}
export function compareTags(a, b) {
  const left = version(a), right = version(b)
  for(let i=0;i<3;i++) if(left.numbers[i] !== right.numbers[i]) return left.numbers[i] > right.numbers[i] ? 1 : -1
  if(!left.pre.length || !right.pre.length) return Number(!left.pre.length) - Number(!right.pre.length)
  for(let i=0;i<Math.max(left.pre.length,right.pre.length);i++) {
    const x=left.pre[i], y=right.pre[i]
    if(x===undefined || y===undefined) return x===undefined ? -1 : 1
    if(x===y) continue
    const nx=/^\d+$/.test(x), ny=/^\d+$/.test(y)
    if(nx && ny) return BigInt(x)>BigInt(y)?1:-1
    if(nx!==ny) return nx?-1:1
    return x>y?1:-1
  }
  return 0
}
export function normalizeRelease(release) {
  const parsed = version(release.tag_name)
  if(release.draft || Boolean(release.prerelease)!==Boolean(parsed.pre.length)) throw new Error('Release channel mismatch')
  if(!release.published_at || !Number.isFinite(Date.parse(release.published_at))) throw new Error('Missing publication date')
  const assets=platforms.map(suffix=>{
    const matches=release.assets.filter(asset=>asset.name.endsWith(suffix))
    if(matches.length!==1) throw new Error('Required installers are not all uploaded')
    const asset=matches[0]
    if(asset.state!=='uploaded' || !Number.isSafeInteger(asset.size) || asset.size<=0 || asset.size>300*1024*1024) throw new Error('Invalid installer')
    if(asset.browser_download_url!==`${prefix}${release.tag_name}/${asset.name}` || !/^onTime_[0-9A-Za-z.+_-]+$/.test(asset.name)) throw new Error('Invalid download URL')
    if(!/^sha256:[0-9a-f]{64}$/i.test(asset.digest??'')) throw new Error('Missing installer SHA-256')
    return {name:asset.name,browser_download_url:asset.browser_download_url,size:asset.size,digest:asset.digest.toLowerCase()}
  })
  return {tag_name:release.tag_name,body:release.body??'',draft:false,prerelease:release.prerelease,published_at:release.published_at,assets}
}
export function buildManifest(releases) {
  const complete=[]
  for(const release of releases) { try { complete.push(normalizeRelease(release)) } catch { /* Unpublished or incomplete releases are excluded. */ } }
  complete.sort((a,b)=>compareTags(b.tag_name,a.tag_name))
  const stable=complete.find(release=>!release.prerelease)??null
  const preview=complete.find(release=>release.prerelease)??null
  if(!stable && !preview) throw new Error('No complete published release with checksums')
  return {schemaVersion:1,generatedAt:new Date().toISOString(),stable,preview}
}
async function api(path, allowMissing=false) {
  const response=await fetch(`https://api.github.com/repos/yeyongzhi/onTime${path}`,{headers:{Accept:'application/vnd.github+json','User-Agent':'onTime-manifest-publisher',...(process.env.GITHUB_TOKEN?{Authorization:`Bearer ${process.env.GITHUB_TOKEN}`}:{})},signal:AbortSignal.timeout(20000)})
  if(allowMissing && response.status===404) return null
  if(!response.ok) throw new Error(`GitHub release metadata request failed (${response.status})`)
  return response.json()
}
async function main() {
  const args=process.argv.slice(2), option=name=>args.includes(name)?args[args.indexOf(name)+1]:undefined
  const input=args.includes('--input')?option('--input'):null
  const expected=args.includes('--wait-for')?option('--wait-for'):null
  if(!input && expected) {
    const deadline=Date.now()+Number(option('--wait-seconds')??1200)*1000
    for(;;) {
      const release=await api(`/releases/tags/${encodeURIComponent(expected)}`,true)
      let ready=false; try { if(release) { normalizeRelease(release); ready=true } } catch { /* Wait for the upload job. */ }
      if(ready) break
      if(Date.now()>=deadline) throw new Error(`Timed out waiting for complete published release ${expected}; existing site remains deployed`)
      console.log(`Waiting for complete published installers: ${expected}`)
      await new Promise(resolve=>setTimeout(resolve,30000))
    }
  }
  let releases=[]
  if(input) releases=JSON.parse(readFileSync(input,'utf8'))
  else for(let page=1;page<=100;page++) {
    const batch=await api(`/releases?per_page=100&page=${page}`)
    releases.push(...batch)
    if(batch.length<100) break
    if(page===100) throw new Error('Release listing exceeds supported limit')
  }
  const manifest=buildManifest(releases)
  const output=args.includes('--output')?option('--output'):'docs/.vitepress/dist/updates.json'
  mkdirSync(dirname(output),{recursive:true}); writeFileSync(output,JSON.stringify(manifest,null,2)+'\n')
  console.log(`Published manifest: stable=${manifest.stable?.tag_name??'none'}, preview=${manifest.preview?.tag_name??'none'}`)
}
if(process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) await main()
