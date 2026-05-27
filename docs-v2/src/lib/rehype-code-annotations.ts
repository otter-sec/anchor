import type { Root } from 'hast'
import { parseCodeAnnotations } from './code-annotations'
import { hasClass, isElement, textContent, visitHtml } from './html-tree'

export function rehypeCodeAnnotations() {
  return (tree: Root) => {
    visitHtml(tree, (node, _index, parent) => {
      if (!isElement(node)) return
      if (node.tagName === 'pre') return 'skip'
      if (node.tagName !== 'code') return
      if (hasClass(node, 'annotated-code')) return 'skip'
      if (parent && isElement(parent) && parent.tagName === 'pre') return 'skip'
      if (parent && isElement(parent) && hasClass(parent, 'shiki')) return 'skip'

      const children = parseCodeAnnotations(textContent(node))
      if (children) {
        const classes = Array.isArray(node.properties.className)
          ? node.properties.className.map(String)
          : []
        if (!classes.includes('annotated-code')) classes.push('annotated-code')
        if (!classes.includes('code-with-tones')) classes.push('code-with-tones')
        node.properties.className = classes
        node.children = children
      }
      return 'skip'
    })
  }
}
