// The ripwire-broker mod (spec/plan/mod-plan.md, Etapa 2). Where mods load, it answers the
// three moments through the MCP server the session already has connected, instead of a
// process per event; the classic hooks in hooks.json stay for where mods do not load.

export function register(on) {
  on('session.start', async ($, e, next) => {
    // The classic hooks run scripts/broker, which does nothing while this is set (DM-5):
    // one plugin, never two injections.
    await $.env.set('RIPWIRE_BROKER_MOD_ACTIVE', '1')
    return next(e)
  })
}
