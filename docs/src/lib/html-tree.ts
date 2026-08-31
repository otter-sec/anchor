import type { Element, ElementContent, Root } from 'hast'

export type HtmlNode = Root | ElementContent
export type HtmlParent = Root | Element
export type HtmlVisitAction = 'skip' | void

export function isElement(node: HtmlNode): node is Element {
  return node.type === 'element'
}

export function classNames(node: Element): string[] {
  const value = node.properties?.className ?? node.properties?.class

  if (Array.isArray(value)) return value.map(String)
  if (typeof value === 'string') return value.split(/\s+/).filter(Boolean)

  return []
}

export function hasClass(node: Element, className: string): boolean {
  return classNames(node).includes(className)
}

export function textContent(node: ElementContent): string {
  if (node.type === 'text') return node.value
  if (!isElement(node)) return ''

  return node.children.map(textContent).join('')
}

export function findChildElement(node: Element, tagName: string): Element | null {
  return (
    node.children.find(
      (child): child is Element => isElement(child) && child.tagName === tagName,
    ) ?? null
  )
}

export function replaceChild(parent: HtmlParent, index: number, child: ElementContent): void {
  const children = parent.children as ElementContent[]
  children[index] = child
}

function childNodes(node: HtmlNode): ElementContent[] | null {
  const children = (node as { children?: unknown }).children
  if (Array.isArray(children)) return children as ElementContent[]

  return null
}

export function visitHtml(
  tree: Root,
  visitor: (
    node: HtmlNode,
    index: number | undefined,
    parent: HtmlParent | undefined,
  ) => HtmlVisitAction,
): void {
  const walk = (
    node: HtmlNode,
    index: number | undefined,
    parent: HtmlParent | undefined,
  ): void => {
    if (visitor(node, index, parent) === 'skip') return

    const children = childNodes(node)
    if (!children) return

    for (let i = 0; i < children.length; i++) {
      walk(children[i], i, node as HtmlParent)
    }
  }

  walk(tree, undefined, undefined)
}
