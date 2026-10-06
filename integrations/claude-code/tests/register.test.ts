// The mod (hooks/register.ts) through Claude Code's test kit: no session, no network
// (spec/plan/mod-plan.md, Etapa 2). Run with `claude plugin test integrations/claude-code`.
import { expect, test } from 'claude-code/testing'

test('session.start marks the mod active for the classic hooks', async ($, on) => {
  const set: Array<{ name: string; value?: string }> = []
  on('env.set', ($, e) => {
    set.push({ name: e.name, value: e.value })
    return { value: undefined }
  })
  on('session.start', () => ({ cwd: '/work' }))
  await $.session.start({ surface: 'terminal', isInteractive: true, cwd: '/work' })
  // scripts/broker exits 0 without running anything while it is set (DM-5, tests/plugin.rs).
  expect(set).toContainEqual({ name: 'RIPWIRE_BROKER_MOD_ACTIVE', value: '1' })
})
