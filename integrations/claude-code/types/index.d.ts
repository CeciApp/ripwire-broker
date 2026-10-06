// The mod's session state ($.state): it survives a reload of the module, which a variable does
// not, and is reset by /clear, /resume and /branch, as the classic hook's session file is per
// session (spec/plan/mod-plan.md, Fase 5).
declare module 'claude-code' {
  interface PluginState {
    'ripwire-broker': {
      /** Prompts seen this session: only the first gets context, unless every_prompt. */
      prompts_seen: number
      /** `#ripwire-off` was typed and no `#ripwire-on` since. */
      opted_out: boolean
    }
  }
}
