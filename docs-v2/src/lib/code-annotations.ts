import type { Element, ElementContent, Properties, Text } from 'hast'

const TONE_TAGS = [
  'black',
  'red',
  'green',
  'orange',
  'yellow',
  'blue',
  'magenta',
  'cyan',
  'white',
  'gray',
  'grey',
  'muted',
  'dim',
  'dimmed',
  'bold',
  'italic',
] as const
type ToneTag = (typeof TONE_TAGS)[number]

const ROUTE_TAG = 'route'
const RESPONSE_TAG = 'response'

const TONE_ALIASES = new Map<string, ToneTag>([
  ['gray', 'black'],
  ['grey', 'black'],
  ['muted', 'black'],
])

const TONE_COLORS = new Map<string, string>([
  ['black', 'var(--muted-foreground)'],
  ['red', 'var(--inline-tone-red)'],
  ['green', 'var(--inline-tone-green)'],
  ['orange', 'var(--inline-tone-orange)'],
  ['yellow', 'var(--inline-tone-yellow)'],
  ['blue', 'var(--inline-tone-blue)'],
  ['magenta', 'var(--inline-tone-magenta)'],
  ['cyan', 'var(--inline-tone-cyan)'],
  ['white', 'color-mix(in oklab, var(--foreground) 88%, transparent)'],
])

const tagRegex = (tags: readonly string[]) => new RegExp(`<\\/?(${tags.join('|')})>`, 'g')
export const CODE_ANNOTATION_TAGS = [ROUTE_TAG, RESPONSE_TAG, ...TONE_TAGS] as const

const CODE_ANNOTATION_TAG_RE = tagRegex(CODE_ANNOTATION_TAGS)
const CODE_TONE_TAG_RE = tagRegex(TONE_TAGS)

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const
type Method = (typeof METHODS)[number]

const METHOD_TONES = new Map<string, ToneTag>([
  ['GET', 'cyan'],
  ['POST', 'green'],
  ['PUT', 'magenta'],
  ['PATCH', 'orange'],
  ['DELETE', 'red'],
  ['HEAD', 'blue'],
  ['OPTIONS', 'blue'],
] satisfies [Method, ToneTag][])

export function methodToneTag(method: string): ToneTag {
  return METHOD_TONES.get(method.toUpperCase()) ?? 'white'
}

const METHOD_PATTERN = METHODS.join('|')
const METHOD_GROUP_RE = new RegExp(`^(?:${METHOD_PATTERN})(?:/(?:${METHOD_PATTERN}))*$`)
const ROUTE_RE = new RegExp(`^((?:${METHOD_PATTERN})(?:/(?:${METHOD_PATTERN}))*)\\s+(.+)$`)
const RESPONSE_RE = /^([1-5](?:\d{2}|XX))(?:\s+(.+))?$/i

export function textNode(value: string): Text {
  return { type: 'text', value }
}

function spanNode(
  className: string[],
  children: ElementContent[],
  properties: Properties = {},
): Element {
  return {
    type: 'element',
    tagName: 'span',
    properties: { ...properties, className },
    children,
  }
}

function normalizeTone(tag: string): string {
  return TONE_ALIASES.get(tag) ?? tag
}

export function toneClassNames(tag: string): string[] {
  return ['code-tone', ...tag.split('+').map((tone) => `is-${normalizeTone(tone)}`)]
}

export function toneStyle(tag: string): string | undefined {
  const tones = tag.split('+').map(normalizeTone)
  const declarations: string[] = []
  const color = tones.map((tone) => TONE_COLORS.get(tone)).find(Boolean)

  if (color) declarations.push(`color: ${color}`)
  if (tones.some(isDimTone)) declarations.push('opacity: 0.5')
  if (tones.includes('bold')) declarations.push('font-weight: 600')
  if (tones.includes('italic')) declarations.push('font-style: italic')

  return declarations.length > 0 ? declarations.join('; ') : undefined
}

function toneNode(tag: string, children: ElementContent[]): Element {
  return spanNode(toneClassNames(tag), children)
}

function isDimTone(tone: string): boolean {
  return tone === 'dim' || tone === 'dimmed'
}

function methodNodes(methods: string): ElementContent[] {
  return methods.split('/').flatMap((method, i) => {
    const node = toneNode(methodToneTag(method), [textNode(method)])
    return i === 0 ? [node] : [textNode('/'), node]
  })
}

function routePathNodes(path: string): ElementContent[] {
  let prefix = ''
  let rest = path
  if (path === '/api' || path.startsWith('/api/') || path.startsWith('/api#')) {
    prefix = path.startsWith('/api/') ? '/api/' : '/api'
    rest = path.slice(prefix.length)
  } else if (path.startsWith('/')) {
    prefix = '/'
    rest = path.slice(1)
  }

  const nodes: ElementContent[] = []
  if (prefix) nodes.push(toneNode('black', [textNode(prefix)]))
  for (const [i, segment] of rest.split('/').entries()) {
    if (i > 0) nodes.push(toneNode('black', [textNode('/')]))
    if (segment) nodes.push(textNode(segment))
  }
  return nodes
}

function routeChildren(value: string): ElementContent[] {
  const route = value.trim()
  if (METHOD_GROUP_RE.test(route)) return methodNodes(route)

  const endpoint = route.match(ROUTE_RE)
  if (endpoint)
    return [...methodNodes(endpoint[1]!), textNode(' '), ...routePathNodes(endpoint[2]!)]

  if (route.startsWith('/')) return routePathNodes(route)
  return [textNode(value)]
}

function responseTone(status: string): ToneTag {
  if (status.startsWith('2')) return 'green'
  if (status.startsWith('3')) return 'orange'
  if (status.startsWith('4') || status.startsWith('5')) return 'red'
  return 'white'
}

function responseChildren(value: string): ElementContent[] {
  const response = value.trim()
  const match = response.match(RESPONSE_RE)
  if (!match) return [textNode(value)]

  const status = match[1]
  if (!status) return [textNode(value)]

  const kind = match[2]
  const tone = responseTone(status)

  return [
    toneNode(tone, [
      toneNode(`${tone}+dim`, [textNode(status)]),
      ...(kind ? [textNode(` ${kind}`)] : []),
    ]),
  ]
}

function isRouteText(value: string): boolean {
  const route = value.trim()
  return METHOD_GROUP_RE.test(route) || ROUTE_RE.test(route) || route.startsWith('/')
}

export function routeCodeNode(value: string): Element | null {
  if (!isRouteText(value)) return null
  return {
    type: 'element',
    tagName: 'code',
    properties: { className: ['annotated-code', 'code-with-tones'] },
    children: [spanNode(['code-route'], routeChildren(value))],
  }
}

export function stripCodeAnnotationTags(value: string): string {
  return value.replace(CODE_ANNOTATION_TAG_RE, '')
}

interface ParsedText {
  type: 'text'
  value: string
}

interface ParsedTag {
  type: 'tag'
  tag: string
  start: number
  end: number
  children: ParsedNode[]
}

type ParsedNode = ParsedText | ParsedTag

interface ParsedTree {
  text: string
  children: ParsedNode[]
}

interface Frame {
  tag: string | null
  start: number
  children: ParsedNode[]
}

function parseTagTree(input: string, re: RegExp): ParsedTree | null {
  const root: Frame = { tag: null, start: 0, children: [] }
  const stack: Frame[] = [root]
  let text = ''
  let lastIndex = 0
  let sawTag = false

  const pushText = (value: string): void => {
    if (!value) return
    stack[stack.length - 1]!.children.push({ type: 'text', value })
    text += value
  }

  for (const match of input.matchAll(re)) {
    const token = match[0]
    const tag = match[1]
    if (!tag) return null
    const index = match.index ?? 0

    pushText(input.slice(lastIndex, index))
    sawTag = true
    lastIndex = index + token.length

    if (token.startsWith('</')) {
      const frame = stack.pop()
      if (!frame || frame.tag !== tag) return null
      stack[stack.length - 1]!.children.push({
        type: 'tag',
        tag,
        start: frame.start,
        end: text.length,
        children: frame.children,
      })
    } else {
      stack.push({ tag, start: text.length, children: [] })
    }
  }

  if (!sawTag || stack.length !== 1) return null
  pushText(input.slice(lastIndex))
  return { text, children: root.children }
}

function flatText(nodes: ParsedNode[]): string {
  return nodes.map((n) => (n.type === 'text' ? n.value : flatText(n.children))).join('')
}

function renderNodes(nodes: ParsedNode[], parentTone: string | null = null): ElementContent[] {
  return nodes.map((node) => {
    if (node.type === 'text') return textNode(node.value)
    if (node.tag === ROUTE_TAG) {
      return spanNode(['code-route'], routeChildren(flatText(node.children)))
    }
    if (node.tag === RESPONSE_TAG) {
      return spanNode(['code-response'], responseChildren(flatText(node.children)))
    }
    const tone = normalizeTone(node.tag)
    const classTone = isDimTone(tone) && parentTone ? `${parentTone}+${tone}` : tone
    const childTone = isDimTone(tone) ? parentTone : tone
    return toneNode(classTone, renderNodes(node.children, childTone))
  })
}

export function parseCodeAnnotations(value: string): ElementContent[] | null {
  const parsed = parseTagTree(value, CODE_ANNOTATION_TAG_RE)
  return parsed && renderNodes(parsed.children)
}

export interface CodeToneRange {
  tone: string
  start: number
  end: number
}

export interface ParsedCodeTones {
  text: string
  ranges: CodeToneRange[]
}

export function parseCodeToneRanges(value: string): ParsedCodeTones | null {
  const parsed = parseTagTree(value, CODE_TONE_TAG_RE)
  if (!parsed) return null

  const ranges: CodeToneRange[] = []
  const walk = (nodes: ParsedNode[], parentTone: string | null = null): void => {
    for (const node of nodes) {
      if (node.type === 'text') continue
      const tone = normalizeTone(node.tag)
      const rangeTone = isDimTone(tone) && parentTone ? `${parentTone}+${tone}` : tone
      if (node.start < node.end) {
        ranges.push({ tone: rangeTone, start: node.start, end: node.end })
      }
      walk(node.children, isDimTone(tone) ? parentTone : tone)
    }
  }
  walk(parsed.children)
  return { text: parsed.text, ranges }
}
