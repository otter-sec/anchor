import type { Element, ElementContent, Root } from 'hast'
import { findChildElement, hasClass, isElement, visitHtml } from './html-tree'

type CodePathKind = 'file' | 'folder'

function pathKind(node: Element): CodePathKind | null {
  const kind = node.properties['data-code-path-kind']
  return kind === 'file' || kind === 'folder' ? kind : null
}

function stripCodePathHint(node: Element): CodePathKind | null {
  if (node.children.length !== 1) return null
  const child = node.children[0]
  if (!child || child.type !== 'text') return null

  const hint = child.value.match(/\{:(dir|file)\}$/)
  if (!hint) return null

  const kind = hint[1] === 'dir' ? 'folder' : 'file'
  child.value = child.value.slice(0, hint.index)
  node.properties['data-code-path-kind'] = kind
  return kind
}

function addCodePathIcon(node: Element, kind: CodePathKind): void {
  const first = node.children?.[0]
  if (first?.type === 'element' && first.tagName === 'svg' && hasClass(first, 'code-path-icon')) {
    return
  }

  node.children.unshift({
    type: 'element',
    tagName: 'svg',
    properties: {
      viewBox: '0 0 24 24',
      fill: 'none',
      stroke: 'currentColor',
      strokeWidth: 2,
      strokeLinecap: 'round',
      strokeLinejoin: 'round',
      'aria-hidden': 'true',
      className: ['code-path-icon', `is-${kind}`],
    },
    children:
      kind === 'folder'
        ? [
            {
              type: 'element',
              tagName: 'path',
              properties: {
                d: 'M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z',
              },
              children: [],
            },
          ]
        : [
            {
              type: 'element',
              tagName: 'path',
              properties: {
                d: 'M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z',
              },
              children: [],
            },
            {
              type: 'element',
              tagName: 'path',
              properties: { d: 'M14 2v4a2 2 0 0 0 2 2h4' },
              children: [],
            },
          ],
  })
}

function dimPlaceholders(node: Element): void {
  for (let i = 0; i < node.children.length; i++) {
    const child = node.children[i]
    if (!child) continue
    if (child.type === 'text') {
      const text = child.value
      const matches = [...text.matchAll(/<[a-z][a-z0-9_-]*>/g)]
      if (matches.length === 0) continue
      const replacements: ElementContent[] = []
      let lastIdx = 0
      for (const match of matches) {
        const start = match.index ?? 0
        if (start > lastIdx) {
          replacements.push({ type: 'text', value: text.slice(lastIdx, start) })
        }
        replacements.push({
          type: 'element',
          tagName: 'span',
          properties: { className: ['code-path-placeholder'] },
          children: [{ type: 'text', value: match[0] }],
        })
        lastIdx = start + match[0].length
      }
      if (lastIdx < text.length) {
        replacements.push({ type: 'text', value: text.slice(lastIdx) })
      }
      node.children.splice(i, 1, ...replacements)
      i += replacements.length - 1
    } else if (child.type === 'element') {
      dimPlaceholders(child)
    }
  }
}

export function rehypeCodePathHints() {
  return (tree: Root) => {
    visitHtml(tree, (node) => {
      if (!isElement(node)) return
      if (node.tagName === 'pre') return 'skip'
      if (node.tagName !== 'code') return

      stripCodePathHint(node)
      return 'skip'
    })
  }
}

export function rehypeCodePathIcons() {
  return (tree: Root) => {
    visitHtml(tree, (node) => {
      if (!isElement(node)) return

      if (node.tagName === 'pre') return 'skip'
      if (hasClass(node, 'copyable-shell-command')) return 'skip'

      if (node.tagName === 'span' && hasClass(node, 'shiki')) {
        const codeChild = findChildElement(node, 'code')
        if (codeChild) {
          const kind = pathKind(codeChild) ?? stripCodePathHint(codeChild)
          if (kind) {
            dimPlaceholders(codeChild)
            addCodePathIcon(codeChild, kind)
          }
        }
        return 'skip'
      }

      if (node.tagName === 'code') {
        const kind = pathKind(node) ?? stripCodePathHint(node)
        if (kind) {
          dimPlaceholders(node)
          addCodePathIcon(node, kind)
        }
        return 'skip'
      }
    })
  }
}
