import type { Root } from 'hast'
import { hasClass, isElement, replaceChild, visitHtml } from './html-tree'

export function rehypeTableWrappers() {
  return (tree: Root) => {
    visitHtml(tree, (node, index, parent) => {
      if (!isElement(node)) return
      if (node.tagName !== 'table' || !parent || index === undefined) return
      if (isElement(parent) && parent.tagName === 'TableScrollArea') return
      if (isElement(parent) && hasClass(parent, 'table-wrapper')) return

      replaceChild(parent, index, {
        type: 'element',
        tagName: 'TableScrollArea',
        properties: {},
        children: [node],
      })
      return 'skip'
    })
  }
}
