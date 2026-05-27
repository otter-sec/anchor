export type DocsVersion = 'v1' | 'v2'

export const DOCS_VERSION_LABELS: Record<DocsVersion, string> = {
  v2: '2.0-alpha',
  v1: '1.0.1',
}

const VERSION_ROUTE_ALIASES: Record<DocsVersion, Record<string, string>> = {
  v1: {
    'get-started/first-program': 'get-started/local-development',
    'get-started/migrating-from-v1': 'get-started/local-development',
    'fundamentals/accounts-and-context': 'programs/account-types',
    'fundamentals/account-validation': 'reference/account-constraints',
    'fundamentals/pdas-and-resolution': 'fundamentals/pdas',
    'programs/account-data-model': 'programs/account-types',
    'programs/pod-types': 'programs/zero-copy',
    'programs/borsh-accounts-and-realloc': 'programs/account-space-and-realloc',
    'programs/errors-and-require': 'programs/errors',
    'programs/extensibility': 'programs/account-types',
    'reference/macros-and-attributes': 'reference/account-constraints',
    'reference/account-types': 'programs/account-types',
    'reference/feature-flags': 'reference/anchor-toml',
    'reference/examples-and-benchmarks': 'reference/examples',
    'reference/alpha-limitations': 'reference/index',
    'security/secure-by-default': 'security/footguns',
    'security/production-builds': 'security/verifiable-builds',
    'security/performance-and-optimizations': 'security/verifiable-builds',
    'testing/profiling-and-debugger': 'testing/index',
    'testing/coverage': 'testing/index',
  },
  v2: {
    'get-started/solana-playground': 'get-started/first-program',
    'get-started/local-development': 'get-started/first-program',
    'fundamentals/pdas': 'fundamentals/pdas-and-resolution',
    'programs/account-space-and-realloc': 'programs/borsh-accounts-and-realloc',
    'programs/errors': 'programs/errors-and-require',
    'programs/zero-copy': 'programs/account-data-model',
    'reference/avm': 'reference/cli',
    'reference/rust-to-js-types': 'clients/typescript',
    'reference/examples': 'reference/examples-and-benchmarks',
    'security/sealevel-attacks': 'security/secure-by-default',
    'security/footguns': 'security/secure-by-default',
    'security/verifiable-builds': 'security/production-builds',
    'testing/mollusk': 'testing/litesvm',
  },
}

function idFromVersionPath(version: DocsVersion, relativePath: string): string {
  return relativePath ? `${version}/${relativePath}` : `${version}/index`
}

function sectionFallback(version: DocsVersion, relativePath: string): string {
  const [section] = relativePath.split('/')
  if (!section) return ''
  if (section === 'get-started') {
    return version === 'v2' ? 'get-started/first-program' : 'get-started/local-development'
  }
  return `${section}/index`
}

function versionPathCandidates(version: DocsVersion, relativePath: string): string[] {
  const alias = VERSION_ROUTE_ALIASES[version][relativePath]
  const fallback = sectionFallback(version, relativePath)
  return [relativePath, alias, fallback].filter(
    (candidate): candidate is string => typeof candidate === 'string' && candidate.length > 0,
  )
}

function versionDocIdCandidates(version: DocsVersion, relativePath: string): string[] {
  const baseId = idFromVersionPath(version, relativePath)
  if (!relativePath) return [baseId]

  const ids = [baseId, `${baseId}/index`]
  if (relativePath.endsWith('/index')) {
    ids.push(idFromVersionPath(version, relativePath.slice(0, -'/index'.length)))
  }
  return ids
}

export function candidateDocIdsForVersion(version: DocsVersion, relativePath: string): string[] {
  return Array.from(
    new Set(
      versionPathCandidates(version, relativePath).flatMap((path) =>
        versionDocIdCandidates(version, path),
      ),
    ),
  )
}
