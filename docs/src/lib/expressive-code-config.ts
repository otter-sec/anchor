import { pluginCollapsibleSections } from '@expressive-code/plugin-collapsible-sections'
import { pluginFrames } from '@expressive-code/plugin-frames'
import { pluginLineNumbers } from '@expressive-code/plugin-line-numbers'
import { pluginShiki } from '@expressive-code/plugin-shiki'
import { pluginTextMarkers } from '@expressive-code/plugin-text-markers'
import { pluginOutputSeparators } from './expressive-code-output-separators'
import { pluginShellPrompts } from './expressive-code-shell-prompts'
import { pluginCodeTones } from './expressive-code-tones'

export const expressiveCodeOptionalPlugins = () => [
  pluginCollapsibleSections(),
  pluginLineNumbers(),
  pluginShellPrompts(),
  pluginCodeTones(),
  pluginOutputSeparators(),
]

export const expressiveCodeDefaultPlugins = () => [
  pluginShiki(),
  pluginFrames(),
  pluginTextMarkers(),
]

export const expressiveCodeOptions = {
  plugins: expressiveCodeOptionalPlugins(),
  useDarkModeMediaQuery: false,
  // Themes are pre-adjusted in shiki-themes.ts so Expressive Code and inline
  // Shiki render identical colors. Skip Expressive Code's per-render
  // readjustment, which would otherwise redo the contrast pass and drift
  // tokens away from the inline pills.
  minSyntaxHighlightingColorContrast: 0,
  themeCssSelector: (theme: { name: string }) =>
    `[data-theme="${theme.name === 'catppuccin-latte' ? 'light' : 'dark'}"]`,
  defaultProps: {
    wrap: true,
    showLineNumbers: true,
    collapseStyle: 'collapsible-start' as const,
    overridesByLang: {
      'ansi,bat,bash,batch,cmd,console,powershell,ps,ps1,psd1,psm1,sh,shell,shellscript,shellsession,text,zsh':
        { showLineNumbers: false },
      'yaml,yml,toml,json,json5,jsonc,sql,graphql,markdown,mdx': { showLineNumbers: false },
    },
  },
  styleOverrides: {
    codeFontSize: '0.75rem',
    borderColor: 'var(--border)',
    borderWidth: '2px',
    codeFontFamily: 'var(--font-mono)',
    frames: {
      editorActiveTabForeground: 'var(--muted-foreground)',
      editorActiveTabIndicatorBottomColor: 'transparent',
      editorActiveTabIndicatorTopColor: 'transparent',
      editorTabBarBackground: 'transparent',
      editorTabBarBorderBottomColor: 'transparent',
      frameBoxShadowCssValue: 'none',
      terminalTitlebarBackground: 'transparent',
      terminalTitlebarBorderBottomColor: 'transparent',
      terminalTitlebarForeground: 'var(--muted-foreground)',
    },
    lineNumbers: {
      foreground: 'var(--muted-foreground)',
    },
    collapsibleSections: {
      closedBackgroundColor: 'color-mix(in oklab, var(--foreground) 5%, transparent)',
      closedBorderColor: 'color-mix(in oklab, var(--foreground) 18%, transparent)',
      closedTextColor: 'var(--muted-foreground)',
      openBackgroundColorCollapsible: 'color-mix(in oklab, var(--foreground) 3%, transparent)',
      openBorderColor: 'transparent',
    },
    textMarkers: {
      delBackground: 'color-mix(in oklab, var(--diff-deleted) 22%, transparent)',
      delBorderColor: 'color-mix(in oklab, var(--diff-deleted) 65%, transparent)',
      delDiffIndicatorColor: 'var(--diff-deleted)',
      insBackground: 'color-mix(in oklab, var(--diff-inserted) 22%, transparent)',
      insBorderColor: 'color-mix(in oklab, var(--diff-inserted) 65%, transparent)',
      insDiffIndicatorColor: 'var(--diff-inserted)',
      markBackground: 'color-mix(in oklab, var(--accent) 12%, transparent)',
      markBorderColor: 'color-mix(in oklab, var(--accent) 50%, transparent)',
    },
    uiFontFamily: 'var(--font-sans)',
  },
}
