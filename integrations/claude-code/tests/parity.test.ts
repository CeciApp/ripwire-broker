// T5.4: the mod against hook::plan. tests/hooks.rs::plan_decisions_are_exported_for_the_mod writes
// PARITY, hook::plan's decision for each recorded event of each scenario; here the same events go
// through the mod, which must ask the same tool with the same arguments, or nothing. A difference
// is the mod's bug, never the Rust's.
import { expect, mock, test } from 'claude-code/testing'
import { broker, cwd, envelope, git, logs } from './broker.ts'
import { PARITY } from './parity.ts'

for (const scenario of PARITY) {
  test(`parity: ${scenario.name}`, { options: { every_prompt: scenario.every_prompt } }, async ($, on) => {
    const clock = mock.clock(on, { now: scenario.steps[0].now_ms })
    cwd(on)
    logs(on)
    git(on, [])
    const calls = broker(on, Array.from({ length: 20 }, (_, i) => envelope('any', i + 1)))
    on('prompt.submit', ($, e) => ({ text: e.text }))
    on('turn.complete', () => ({ text: '' }))
    let answer: Record<string, unknown> = { result: 'ok' }
    on('tool.call', () => answer)
    for (const step of scenario.steps) {
      await clock.set(step.now_ms)
      const e: any = step.event
      const before = calls.length
      if (e.hook_event_name === 'UserPromptSubmit') {
        await $.prompt.submit({ text: e.prompt, wait: false, origin: { kind: 'composer' } })
      } else if (e.hook_event_name === 'PostToolUse') {
        answer = { result: e.tool_response }
        await $.tool.call({ tool: e.tool_name, ...e.tool_input })
      } else {
        await $.turn.complete({ turnId: 't', answer: 'done', durationMs: 1, isAborted: false, reason: 'answer', usage: null })
      }
      const asked = calls.slice(before).map((c) => ({ tool: c.tool, args: c.args }))
      const d: any = step.decision
      const expected =
        d.ask === null
          ? []
          : d.ask === 'context_for_task'
            ? [{ tool: d.ask, args: { task: d.task, budget_tokens: d.budget_tokens } }]
            : d.ask === 'context_after_edit'
              ? [{ tool: d.ask, args: { files: d.files, budget_tokens: d.budget_tokens } }]
              : [{ tool: d.ask, args: { budget_tokens: 1800 } }]
      expect({ at: step.now_ms, asked }).toEqual({ at: step.now_ms, asked: expected })
    }
  })
}
