import { ensureColorContrastOnBackground, ExpressiveCodeTheme } from '@expressive-code/core'
import catppuccinLatteRaw from '@shikijs/themes/catppuccin-latte'
import catppuccinMochaRaw from '@shikijs/themes/catppuccin-mocha'

// Same default Expressive Code uses internally. Pre-adjusting against this
// once means EC's per-render adjustment is a no-op (already meets contrast),
// and inline shiki - which has no contrast pipeline - sees the same colors.
const MIN_CONTRAST = 5.5

// `ensureMinSyntaxHighlightingColorContrast` only walks tokenColor settings
// and `editor.foreground`. The 16 ANSI palette entries live in `colors`
// and drive `{:ansi}` rendering for both EC terminals and inline shiki, so
// adjust them with the same algorithm against the same background.
const ANSI_COLOR_KEYS = [
  'terminal.ansiBlack',
  'terminal.ansiRed',
  'terminal.ansiGreen',
  'terminal.ansiYellow',
  'terminal.ansiBlue',
  'terminal.ansiMagenta',
  'terminal.ansiCyan',
  'terminal.ansiWhite',
  'terminal.ansiBrightBlack',
  'terminal.ansiBrightRed',
  'terminal.ansiBrightGreen',
  'terminal.ansiBrightYellow',
  'terminal.ansiBrightBlue',
  'terminal.ansiBrightMagenta',
  'terminal.ansiBrightCyan',
  'terminal.ansiBrightWhite',
] as const

function adjustAnsiPalette(theme: ExpressiveCodeTheme): void {
  for (const key of ANSI_COLOR_KEYS) {
    const color = theme.colors[key]
    if (!color) continue
    theme.colors[key] = ensureColorContrastOnBackground(color, theme.bg, MIN_CONTRAST)
  }
}

function adjusted(raw: unknown): ExpressiveCodeTheme {
  const theme = new ExpressiveCodeTheme(raw as ConstructorParameters<typeof ExpressiveCodeTheme>[0])
  theme.ensureMinSyntaxHighlightingColorContrast(MIN_CONTRAST)
  adjustAnsiPalette(theme)
  return theme
}

export const lightTheme = adjusted(catppuccinLatteRaw)
export const darkTheme = adjusted(catppuccinMochaRaw)
