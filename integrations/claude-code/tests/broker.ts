// Stand-ins for the broker's MCP server and for what Claude Code answers, shared by the mod's
// tests. An envelope is the broker's `ripwire-broker.context/v1` answer (spec §8).

export const SERVER = 'plugin:ripwire-broker:broker'

export type Envelope = Record<string, unknown> & { tool: string; status: string }

/** An envelope with one item, as the broker returns it for `tool`. */
export function envelope(tool: string, request: number, extra: Partial<Envelope> = {}): Envelope {
  return {
    schema_version: 'ripwire-broker.context/v1',
    tool,
    status: 'ready',
    summary: 'found 1 item',
    items: [{ kind: 'symbol', path: 'src/parser.rs', symbol: 'parse', why_included: 'rank 1' }],
    tests: [],
    risks: [],
    limitations: [],
    budget: { estimated_tokens: 120, requested_tokens: 1500, shown: 1, omitted: 0, truncated: false },
    provenance: { request_id: request, broker_version: '0.1.0', workspace: '/work' },
    ...extra,
  }
}

/** Nothing to act on: only limitations (D-130). */
export function empty(tool: string, request: number): Envelope {
  return envelope(tool, request, {
    items: [],
    limitations: [{ kind: 'route_uncertain', detail: 'no clear signal' }],
  })
}

export type Call = { server: string; tool: string; args: Record<string, unknown> }

/**
 * Registers the broker's stubs: `mcp.connect` answers the plugin's server, and `mcp.call` answers
 * each call with the next of `answers` (an envelope, or `{ deny }` for a call that fails), as the
 * broker's text block carries it: the JSON on one line (`mcp::text_of`). Returns the calls made.
 */
export function broker(on, answers: Array<Envelope | { deny: string }>): Call[] {
  const calls: Call[] = []
  on('mcp.connect', () => ({ value: { isConnected: true, server: SERVER } }))
  on('mcp.call', ($, e) => {
    calls.push({ server: e.server, tool: e.tool, args: e.args })
    const next = answers.shift()
    if (next === undefined) return { deny: 'no answer left in the test' }
    if ('deny' in next) return { deny: next.deny }
    return { value: { content: [{ type: 'text', text: JSON.stringify(next) }], isError: false } }
  })
  return calls
}

/** Claude Code's own `prompt.submit`, recording what reached it. */
export function core(on): Array<{ text: string; context?: readonly string[] }> {
  const seen: Array<{ text: string; context?: readonly string[] }> = []
  on('prompt.submit', ($, e) => {
    seen.push({ text: e.text, context: e.context })
    return { text: e.text, context: e.context }
  })
  return seen
}

/** The lines the mod logs for the user, which Claude does not read. */
export function logs(on): string[] {
  const lines: string[] = []
  on('ui.log', ($, e) => {
    lines.push(e.text)
    return { value: undefined }
  })
  return lines
}

export const typed = (text: string) => ({ text, wait: false, origin: { kind: 'composer' as const } })
