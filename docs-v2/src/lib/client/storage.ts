type StorageLike = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>

function storageValue(storage: StorageLike, key: string): string | null {
  try {
    return storage.getItem(key)
  } catch {
    return null
  }
}

export function readStorage(storage: StorageLike, key: string): string | null {
  return storageValue(storage, key)
}

export function writeStorage(storage: StorageLike, key: string, value: string): void {
  try {
    storage.setItem(key, value)
  } catch {}
}

export function removeStorage(storage: StorageLike, key: string): void {
  try {
    storage.removeItem(key)
  } catch {}
}

export function readJsonRecord<T>(
  storage: StorageLike,
  key: string,
  fallback: Record<string, T> = {},
): Record<string, T> {
  const value = storageValue(storage, key)
  if (!value) return fallback

  try {
    const parsed = JSON.parse(value)
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : fallback
  } catch {
    return fallback
  }
}

export function writeJson(storage: StorageLike, key: string, value: unknown): void {
  writeStorage(storage, key, JSON.stringify(value))
}

export function readNumber(storage: StorageLike, key: string): number | null {
  const value = storageValue(storage, key)
  if (value === null) return null

  const number = Number(value)
  return Number.isFinite(number) ? number : null
}
