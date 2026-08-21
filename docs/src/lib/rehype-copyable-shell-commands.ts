import type { Element, Root, Text } from 'hast'
import { parseCodeAnnotations, stripCodeAnnotationTags } from './code-annotations'
import { findChildElement, hasClass, isElement, textContent, visitHtml } from './html-tree'

const PREFIX = '$ '
function firstTextNode(node: Element): Text | null {
  for (const child of node.children) {
    if (child.type === 'text') return child
    if (child.type === 'element') {
      const found = firstTextNode(child)
      if (found) return found
    }
  }
  return null
}

function applyCodeAnnotations(codeNode: Element): void {
  const children = parseCodeAnnotations(textContent(codeNode))
  if (children) codeNode.children = children
}

function convertElementToButton(node: Element, codeNode: Element, command: string): void {
  const copyText = stripCodeAnnotationTags(command)
  node.tagName = 'span'
  node.properties = {
    role: 'button',
    tabIndex: 0,
    className: ['copyable-shell-command'],
    'data-copy': copyText,
    'aria-label': `Copy command: ${copyText}`,
    title: `Copy: ${copyText}`,
    ...(copyText.length > 24 ? { 'data-long-pill': 'true' } : {}),
  }
  node.children = [
    {
      type: 'element',
      tagName: 'span',
      properties: { className: ['shell-prompt'], 'aria-hidden': 'true' },
      children: [{ type: 'text', value: '$' }],
    },
    codeNode,
  ]
}

function tryConvertShikiSpan(node: Element): boolean {
  const codeChild = findChildElement(node, 'code')
  if (!codeChild) return false

  const first = firstTextNode(codeChild)
  if (!first || !first.value.startsWith(PREFIX)) return false

  const command = textContent(codeChild).slice(PREFIX.length)
  if (command.length === 0) return false

  first.value = first.value.slice(PREFIX.length)
  applyCodeAnnotations(codeChild)
  convertElementToButton(node, codeChild, command)
  return true
}

function tryConvertPlainCode(node: Element): boolean {
  const first = firstTextNode(node)
  if (!first || !first.value.startsWith(PREFIX)) return false

  const command = textContent(node).slice(PREFIX.length)
  if (command.length === 0) return false

  first.value = first.value.slice(PREFIX.length)
  const codeNode: Element = {
    ...node,
    properties: { ...node.properties },
    children: [...node.children],
  }
  applyCodeAnnotations(codeNode)
  convertElementToButton(node, codeNode, command)
  return true
}

export function rehypeCopyableShellCommands() {
  return (tree: Root) => {
    visitHtml(tree, (node) => {
      if (!isElement(node)) return

      // Block code: skip the whole subtree (handled by expressive-code-shell-prompts).
      if (node.tagName === 'pre') return 'skip'
      // Idempotency: don't re-wrap.
      if (hasClass(node, 'copyable-shell-command')) return 'skip'

      if (node.tagName === 'span' && hasClass(node, 'shiki')) {
        if (tryConvertShikiSpan(node)) return 'skip'
      } else if (node.tagName === 'code') {
        if (tryConvertPlainCode(node)) return 'skip'
      }
    })
  }
}
