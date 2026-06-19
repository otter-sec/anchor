import { ExpressiveCodeEngine } from '@expressive-code/core'
import { expressiveCodeDefaultPlugins, expressiveCodeOptions } from './expressive-code-config'
import { darkTheme, lightTheme } from './shiki-themes'

let enginePromise: Promise<ExpressiveCodeEngine> | null = null
let stylesPromise: Promise<string> | null = null

export function getExpressiveCodeEngine(): Promise<ExpressiveCodeEngine> {
  if (!enginePromise) {
    enginePromise = (async () => {
      return new ExpressiveCodeEngine({
        themes: [lightTheme, darkTheme],
        ...expressiveCodeOptions,
        plugins: [...expressiveCodeDefaultPlugins(), ...expressiveCodeOptions.plugins],
      })
    })()
  }
  return enginePromise
}

export function getExpressiveCodeStyles(): Promise<string> {
  if (!stylesPromise) {
    stylesPromise = (async () => {
      const engine = await getExpressiveCodeEngine()
      const base = await engine.getBaseStyles()
      const themes = await engine.getThemeStyles()
      return base + themes
    })()
  }
  return stylesPromise
}
