// T5.5: the band above the prompt and /ripwire-status, with src/statusline.rs's labels: the mod's
// own counters (it replaces the classic hooks, whose snapshot feeds the classic status line) and
// the server's file (statusline/server-<key>-<pid>.json, src/server_status.rs).
import type { On } from 'claude-code'
import type { Engine } from 'claude-code/testing'
import { expect, mock, test } from 'claude-code/testing'
import { broker, envelope, logs } from './broker.ts'

const ABOVE = {
  plugin: 'ripwire-broker',
  component: 'AbovePrompt',
  requestId: 'above',
  viewport: { columns: 120, rows: 30 },
  props: { hasSurvey: false, isWorking: false, maxRows: 3, bodyColumns: 120, scroll: { offset: 0, bodyRows: 3 }, view: {} },
} as const

const hex = (s: string) => [...new TextEncoder().encode(s)].map((b) => b.toString(16).padStart(2, '0')).join('')

/** The session runs in /work; the state dir is /state/ripwire-broker; `files` are the server
 *  status files there, by name. */
function machine(on: On, files: Record<string, unknown> = {}) {
  mock.env(on, { XDG_STATE_HOME: '/state', HOME: '/home/u' })
  on('session.cwd', () => ({ value: '/work' }))
  on('fs.stat', ($, e) => ({ value: { kind: 'dir', size: 0, mtimeMs: 0, isLink: false, realPath: e.path } }))
  on('fs.list', ($, e) =>
    e.path === '/state/ripwire-broker/statusline'
      ? { value: Object.keys(files).map((name) => ({ name, kind: 'file' as const, size: 100, mtimeMs: 1, isLink: false })) }
      : { deny: 'no such directory' },
  )
  on('fs.read', ($, e) => {
    const name = e.path.split('/').pop() ?? ''
    return name in files ? { value: JSON.stringify(files[name]) } : { deny: 'no such file' }
  })
  on('ui.render', () => ({ type: 'Text', props: {}, children: ['drawn by Claude Code'] }))
}

async function finishWithAttention($: Engine, on: On) {
  broker(on, [
    envelope('context_before_finish', 3, { status: 'attention_required', risks: [{ kind: 'contract_change' }] }),
  ])
  on('turn.complete', () => ({ text: '' }))
  logs(on)
}

test('the band draws the status segments on terminal and desktop', async ($, on) => {
  mock.clock(on, { now: 1_000_000 })
  machine(on)
  await finishWithAttention($, on)
  await $.turn.complete({ turnId: 't', answer: 'ok', durationMs: 1, isAborted: false, reason: 'answer' })
  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...ABOVE, surface })
    expect(await ui.find({ type: 'Text', text: 'rw-brkr · hooks on · última: atenção · inj 0' })).toBeDefined()
    await ui.unmount()
  }
  const vscode = await $.ui.mount({ ...ABOVE, surface: 'vscode' })
  expect(await vscode.find({ type: 'Text', text: /^rw-brkr/ })).toBeUndefined()
})

test('jev and mem come from the server file', async ($, on) => {
  const now = 1_000_000
  mock.clock(on, { now: now * 1000 })
  const key = hex('/work')
  const status = (pid: number, extra = {}) => ({
    schema_version: 1,
    workspace_key: key,
    pid,
    updated_at: now - 2,
    online: true,
    memory: true,
    // Inside the 5 s window: now - 4 counts, now - 5 does not.
    jev_calls: [[now - 4, 2], [now - 5, 7], [now - 9, 5]],
    mem_reads: [[now, 1]],
    mem_stores: [[now - 2, 2]],
    jev_key: 'ok',
    ...extra,
  })
  machine(on, {
    [`server-${key}-11.json`]: status(11),
    [`server-${key}-12.json`]: status(12, { memory: false, jev_calls: [[now, 1]], mem_reads: [], mem_stores: [] }),
    // Another workspace, and a server gone for more than 30 s: neither counts.
    [`server-${hex('/other')}-13.json`]: status(13, { workspace_key: hex('/other') }),
    [`server-${key}-14.json`]: status(14, { updated_at: now - 31 }),
    // Named for this workspace, but its content says otherwise: the content is what counts.
    [`server-${key}-15.json`]: status(15, { workspace_key: hex('/other') }),
  })
  const ui = await $.ui.mount({ ...ABOVE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: 'rw-brkr · hooks on · inj 0 · [jev:3] · [mem: retr 1, stor 2] · (online)' })).toBeDefined()
})

test('a key the server could not use is shown as such', async ($, on) => {
  const now = 1_000_000
  mock.clock(on, { now: now * 1000 })
  const key = hex('/work')
  machine(on, {
    [`server-${key}-11.json`]: { schema_version: 1, workspace_key: key, pid: 11, updated_at: now, online: true, memory: false, jev_calls: [], mem_reads: [], mem_stores: [], jev_key: 'missing' },
  })
  const ui = await $.ui.mount({ ...ABOVE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: 'rw-brkr · hooks on · inj 0 · [jev: no key] · (online)' })).toBeDefined()
})

test('/ripwire-status prints the same line', async ($, on) => {
  mock.clock(on, { now: 1_000_000 })
  machine(on)
  const registered: string[] = []
  on('command.register', ($, e) => {
    registered.push(e.name)
    return { value: { command: e.name } }
  })
  on('env.set', () => ({ value: undefined }))
  on('session.start', () => ({ cwd: '/work' }))
  on('prompt.submit', ($, e) => ({ text: e.text }))
  logs(on)
  await $.session.start({ surface: 'terminal', isInteractive: true, cwd: '/work' })
  await $.prompt.submit({ text: '#ripwire-off', wait: false, origin: { kind: 'composer' } })
  const answer = await $.command.run({
    command: 'ripwire-status',
    args: '',
    origin: { kind: 'composer' },
    presentation: { isFullscreen: false, columns: 120 },
  })
  expect(registered).toEqual(['ripwire-status'])
  expect(answer.text).toBe('rw-brkr · hooks off · inj 0')
})

test('context that reached Claude counts in inj, and a failure shows as an error', async ($, on) => {
  mock.clock(on, { now: 1_000_000 })
  machine(on)
  broker(on, [envelope('context_for_task', 1), { deny: 'server gone' }])
  on('prompt.submit', ($, e) => ({ text: e.text }))
  on('tool.call', () => ({ result: 'ok' }))
  logs(on)
  await $.prompt.submit({ text: 'fix the parser', wait: false, origin: { kind: 'composer' } })
  let ui = await $.ui.mount({ ...ABOVE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: 'rw-brkr · hooks on · última: pronta · inj 1' })).toBeDefined()
  await ui.unmount()
  await $.tool.call({ tool: 'Edit', file_path: '/work/a.rs', old_string: 'a', new_string: 'b' })
  ui = await $.ui.mount({ ...ABOVE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: 'rw-brkr · hooks on · última: erro · inj 1' })).toBeDefined()
})

test('at most 16 server files are read', async ($, on) => {
  const now = 1_000_000
  mock.clock(on, { now: now * 1000 })
  const key = hex('/work')
  const files: Record<string, unknown> = {}
  for (let pid = 1; pid <= 17; pid++) {
    files[`server-${key}-${pid}.json`] = {
      schema_version: 1, workspace_key: key, pid, updated_at: now, online: true, memory: false,
      jev_calls: [[now, 1]], mem_reads: [], mem_stores: [], jev_key: 'ok',
    }
  }
  machine(on, files)
  const ui = await $.ui.mount({ ...ABOVE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: 'rw-brkr · hooks on · inj 0 · [jev:16] · (online)' })).toBeDefined()
})

test('a failed prompt check shows as an error too', async ($, on) => {
  mock.clock(on, { now: 1_000_000 })
  machine(on)
  broker(on, [{ deny: 'server gone' }])
  on('prompt.submit', ($, e) => ({ text: e.text }))
  logs(on)
  await $.prompt.submit({ text: 'fix the parser', wait: false, origin: { kind: 'composer' } })
  const ui = await $.ui.mount({ ...ABOVE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: 'rw-brkr · hooks on · última: erro · inj 0' })).toBeDefined()
})

test('a failed finish check shows as an error', async ($, on) => {
  mock.clock(on, { now: 1_000_000 })
  machine(on)
  broker(on, [{ deny: 'server gone' }])
  on('turn.complete', () => ({ text: '' }))
  logs(on)
  await $.turn.complete({ turnId: 't', answer: 'ok', durationMs: 1, isAborted: false, reason: 'answer' })
  const ui = await $.ui.mount({ ...ABOVE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: 'rw-brkr · hooks on · última: erro · inj 0' })).toBeDefined()
})

test("the finish gate's prompt counts in inj", { options: { gate: true } }, async ($, on) => {
  mock.clock(on, { now: 1_000_000 })
  machine(on)
  await finishWithAttention($, on)
  on('prompt.submit', ($, e) => ({ text: e.text }))
  await $.turn.complete({ turnId: 't', answer: 'ok', durationMs: 1, isAborted: false, reason: 'answer' })
  const ui = await $.ui.mount({ ...ABOVE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: 'rw-brkr · hooks on · última: atenção · inj 1' })).toBeDefined()
})
