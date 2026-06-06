// docs using vercel's justbash
import { Bash } from 'just-bash'

type Doc = { id: string; title: string; description: string; url: string; body: string }

const PROTOCOL_VERSION = '2025-06-18'
const cache = new Map<string, Doc[]>()

async function getDocs(origin: string): Promise<Doc[]> {
  if (!cache.has(origin)) {
    const url = process.env.MCP_INDEX_URL ?? `${origin}/docs/search-index.json`
    cache.set(origin, await fetch(url).then((r) => r.json()))
  }
  return cache.get(origin)!
}

const TOOLS = [
  {
    name: 'search_anchor_docs',
    description: 'Search the Anchor (anchor-lang) docs. Returns matching pages with title, path, url, snippet.',
    inputSchema: { type: 'object', properties: { query: { type: 'string' } }, required: ['query'] },
  },
  {
    name: 'read_anchor_doc',
    description: 'Read the full Markdown of an Anchor docs page by its path (from search results), e.g. "v2/fundamentals/pda".',
    inputSchema: { type: 'object', properties: { path: { type: 'string' } }, required: ['path'] },
  },
  {
    name: 'query_docs_filesystem_anchor',
    description:
      'Run a read-only shell command (rg, grep, find, tree, ls, cat, head, sed, awk, jq, pipes, &&) against an in-memory filesystem of the Anchor docs rooted at /. Pages are at /<path>.mdx. No network, no persisted writes.',
    inputSchema: { type: 'object', properties: { command: { type: 'string' } }, required: ['command'] },
  },
]

function search(docs: Doc[], query: string, limit = 8) {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean)
  return docs
    .map((d) => {
      const hay = `${d.title}\n${d.description}\n${d.body}`.toLowerCase()
      let score = 0
      for (const t of terms) {
        if (d.title.toLowerCase().includes(t)) score += 10
        if (d.description.toLowerCase().includes(t)) score += 4
        score += Math.min(hay.split(t).length - 1, 5)
      }
      return { d, score }
    })
    .filter((r) => r.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map((r) => r.d)
}

async function shell(docs: Doc[], command: string): Promise<string> {
  const files = Object.fromEntries(docs.map((d) => [`/${d.id}.mdx`, `Title: ${d.title}\n${d.description}\n\n${d.body}`]))
  //this is a fake shell to make it easier for the agent to interact, nothing too fancy
  const { stdout, stderr, exitCode } = await new Bash({
    files,
    cwd: '/',
    executionLimits: { maxCommandCount: 200, maxLoopIterations: 10_000 },
  }).exec(command)
  return `exit: ${exitCode}` + (stdout ? `\n--- stdout ---\n${stdout}` : '') + (stderr ? `\n--- stderr ---\n${stderr}` : '')
}

const ok = (id: unknown, result: unknown) =>
  Response.json({ jsonrpc: '2.0', id, result }, { headers: { 'Cache-Control': 'no-store' } })
const fail = (id: unknown, code: number, message: string) =>
  Response.json({ jsonrpc: '2.0', id, error: { code, message } })

export async function POST(request: Request) {
  let m: any
  try {
    m = await request.json()
  } catch {
    return fail(null, -32700, 'Parse error')
  }
  if (m.id === undefined || m.id === null) return new Response(null, { status: 202 }) // notification

  if (m.method === 'initialize')
    return ok(m.id, {
      protocolVersion: PROTOCOL_VERSION,
      capabilities: { tools: { listChanged: false } },
      serverInfo: { name: 'Anchor Docs', version: '1.0.0' },
    })
  if (m.method === 'ping') return ok(m.id, {})
  if (m.method === 'tools/list') return ok(m.id, { tools: TOOLS })
  if (m.method === 'tools/call') {
    const { name, arguments: args = {} } = m.params ?? {}
    const docs = await getDocs(new URL(request.url).origin)

    if (name === 'search_anchor_docs') {
      const q = String(args.query ?? '')
      const hits = search(docs, q)
      const text = hits.length
        ? hits.map((d) => `## ${d.title}\npath: ${d.id}\nurl: ${d.url}\n${d.description || d.body.slice(0, 200)}`).join('\n\n')
        : `No results for "${q}".`
      return ok(m.id, { content: [{ type: 'text', text }] })
    }
    if (name === 'read_anchor_doc') {
      const path = String(args.path ?? '').replace(/^\/+|\/+$|\.mdx?$/g, '')
      const d = docs.find((x) => x.id === path || x.id === `${path}/index`)
      return ok(m.id, {
        content: [{ type: 'text', text: d ? `# ${d.title}\n${d.url}\n\n${d.body}` : `Not found: ${path}` }],
        isError: !d,
      })
    }
    if (name === 'query_docs_filesystem_anchor')
      return ok(m.id, { content: [{ type: 'text', text: await shell(docs, String(args.command ?? '')) }] })

    return fail(m.id, -32602, `Unknown tool: ${name}`)
  }
  return fail(m.id, -32601, `Method not found: ${m.method}`)
}

export function GET() {
  return new Response('Method Not Allowed', { status: 405, headers: { Allow: 'POST' } })
}
