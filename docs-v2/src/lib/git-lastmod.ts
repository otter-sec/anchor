import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'

const cache = new Map<string, Date | null>()

export function gitLastUpdated(filePath: string): Date | null {
  if (cache.has(filePath)) return cache.get(filePath)!

  let result: Date | null = null
  try {
    const abs = resolve(filePath)
    const output = execFileSync('git', ['log', '-1', '--format=%ct', '--', abs], {
      encoding: 'utf-8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim()
    if (output) result = new Date(Number.parseInt(output, 10) * 1000)
  } catch {
    result = null
  }

  cache.set(filePath, result)
  return result
}
