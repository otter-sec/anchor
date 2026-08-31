import {
  definePlugin,
  ExpressiveCodeAnnotation,
  type AnnotationRenderOptions,
  type ExpressiveCodePlugin,
} from '@expressive-code/core'
import { h, type Element, type Parents } from '@expressive-code/core/hast'
import { parseCodeToneRanges, toneClassNames, toneStyle } from './code-annotations'

function stripElementSyntaxStyles(node: Element): Element {
  const {
    class: _class,
    className: _className,
    style: _style,
    ...properties
  } = node.properties ?? {}
  return {
    ...node,
    properties,
    children: node.children.map((child) =>
      child.type === 'element' ? stripElementSyntaxStyles(child) : child,
    ),
  }
}

class CodeToneAnnotation extends ExpressiveCodeAnnotation {
  readonly name = 'code-tone'
  private readonly tone: string

  constructor(tone: string, start: number, end: number) {
    super({
      inlineRange: { columnStart: start, columnEnd: end },
      renderPhase: 'latest',
    })
    this.tone = tone
  }

  render({ nodesToTransform }: AnnotationRenderOptions): Parents[] {
    const style = toneStyle(this.tone)
    return nodesToTransform.map((node) =>
      h(
        'span',
        {
          className: toneClassNames(this.tone),
          ...(style ? { style } : {}),
        },
        node.type === 'element' ? stripElementSyntaxStyles(node) : node,
      ),
    )
  }
}

export function pluginCodeTones(): ExpressiveCodePlugin {
  return definePlugin({
    name: 'Code Tones',
    hooks: {
      preprocessCode: ({ codeBlock }) => {
        for (const line of codeBlock.getLines()) {
          const parsed = parseCodeToneRanges(line.text)
          if (!parsed) continue

          line.editText(0, line.text.length, parsed.text)
          for (const range of parsed.ranges) {
            line.addAnnotation(new CodeToneAnnotation(range.tone, range.start, range.end))
          }
        }
      },
    },
  })
}
