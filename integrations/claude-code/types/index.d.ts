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
      /** Files edited inside the one-second window, sent with the next edit asked about. */
      held_edits: string[]
      /** When the last edit was asked about, ms since the epoch; 0 for never. */
      last_edit_ms: number
      /** Slow `git status` snapshots in a row. */
      slow_statuses: number
      /** Bash edit detection is off for the session: git is too slow or the tree too dirty. */
      worktree_off: boolean
      /** The host has said which files a Bash changed (`bashEditDiff`, D-131): from then on its
       *  list is the answer, its absence means nothing changed, and git is not asked again. */
      host_reports_bash_edits: boolean
      /** The running turn was started by the finish gate's prompt: it is not gated again, as a
       *  Stop hook with `stop_hook_active` is not. */
      looping_turn: boolean
      /** The text the finish gate submitted and has not seen come back yet: the engine stamps
       *  its origin, the test kit does not, and either way the prompt hook lets it pass. */
      gate_prompt: string
    }
  }
}
