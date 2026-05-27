import type { Element, ElementContent, Root } from 'hast'
import { hasClass, isElement, visitHtml } from './html-tree'

function buildIcon(): Element {
  return {
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
      focusable: 'false',
      className: ['link-icon'],
    },
    children: [
      {
        type: 'element',
        tagName: 'path',
        properties: { d: 'M9 15l6 -6' },
        children: [],
      },
      {
        type: 'element',
        tagName: 'path',
        properties: { d: 'M11 6l.463 -.536a5 5 0 0 1 7.071 7.072l-.534 .464' },
        children: [],
      },
      {
        type: 'element',
        tagName: 'path',
        properties: {
          d: 'M13 18l-.397 .534a5.068 5.068 0 0 1 -7.127 0a4.972 4.972 0 0 1 0 -7.071l.524 -.463',
        },
        children: [],
      },
    ],
  }
}

function isAlreadyProcessed(link: Element): boolean {
  for (let i = link.children.length - 1; i >= 0; i--) {
    const child = link.children[i]
    if (!isElement(child)) continue
    if (hasClass(child, 'link-icon')) return true
    for (const inner of child.children) {
      if (isElement(inner) && hasClass(inner, 'link-icon')) return true
    }
    return false
  }
  return false
}

function appendIconWithLastWord(link: Element): void {
  const icon = buildIcon()
  const children = link.children

  let lastIdx = children.length - 1
  while (lastIdx >= 0) {
    const child = children[lastIdx]
    if (child.type === 'text' && child.value.trim() === '') {
      lastIdx--
      continue
    }
    break
  }

  if (lastIdx < 0) {
    children.push(icon)
    return
  }

  const last = children[lastIdx]

  if (last.type === 'text') {
    const match = last.value.match(/^([\s\S]*?)(\S+)(\s*)$/)
    if (!match) {
      children.push(icon)
      return
    }
    const [, before, lastWord, after] = match
    const wrap: Element = {
      type: 'element',
      tagName: 'span',
      properties: { style: 'white-space: nowrap' },
      children: [{ type: 'text', value: lastWord }, icon],
    }
    const replacement: ElementContent[] = []
    if (before.length > 0) replacement.push({ type: 'text', value: before })
    replacement.push(wrap)
    if (after.length > 0) replacement.push({ type: 'text', value: after })
    children.splice(lastIdx, 1, ...replacement)
    return
  }

  if (isElement(last)) {
    const wrap: Element = {
      type: 'element',
      tagName: 'span',
      properties: { style: 'white-space: nowrap' },
      children: [last, icon],
    }
    children.splice(lastIdx, 1, wrap)
    return
  }

  children.push(icon)
}

export function rehypeLinkIcons() {
  return (tree: Root) => {
    visitHtml(tree, (node) => {
      if (!isElement(node)) return

      if (node.tagName === 'pre') return 'skip'
      if (hasClass(node, 'link-icon')) return 'skip'
      if (node.tagName !== 'a') return
      if (hasClass(node, 'heading-anchor')) return 'skip'
      if (typeof node.properties.href !== 'string' || node.properties.href.length === 0) return
      if (isAlreadyProcessed(node)) return 'skip'

      appendIconWithLastWord(node)
      return 'skip'
    })
  }
}
