import { readFileSync, writeFileSync, appendFileSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'

const source = readFileSync('release.yml', 'utf8')
const scalar = name => source.match(new RegExp(`^${name}: (.+)$`, 'm'))?.[1].trim()
const tag = scalar('tag')
const version = JSON.parse(readFileSync('package.json', 'utf8')).version
if (tag !== `v${version}` || (process.env.GITHUB_REF_NAME && process.env.GITHUB_REF_NAME !== tag)) {
  throw new Error('Release tag and package version must match')
}
const body = source.split(/^body: \|\r?\n/m)[1]?.replace(/^  /gm, '')
if (!body?.startsWith('发布日期：') || !scalar('title')) throw new Error('Invalid release notes')
const bodyPath = join(process.env.RUNNER_TEMP ?? tmpdir(), 'ontime-release-body.md')
writeFileSync(bodyPath, body)
if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT,
  `title=${scalar('title')}\nprerelease=${scalar('prerelease')}\nbody_path=${bodyPath}\n`)
console.log(`Prepared release notes for ${tag}`)
