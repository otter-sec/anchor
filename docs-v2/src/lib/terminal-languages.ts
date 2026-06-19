const TERMINAL_LANGUAGES = new Set([
  'ansi',
  'bash',
  'sh',
  'shell',
  'shellscript',
  'shellsession',
  'zsh',
  'console',
])

export function isTerminalLanguage(language: string): boolean {
  return TERMINAL_LANGUAGES.has(language)
}
