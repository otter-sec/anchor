import { SITE } from '@/consts'
import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function formatDate(date: Date) {
  return Intl.DateTimeFormat(SITE.locale, {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  }).format(date)
}

export function formatRelativeDate(date: Date, now: Date = new Date()): string {
  const diffMs = now.getTime() - date.getTime()
  const diffSec = Math.round(diffMs / 1000)
  const diffMin = Math.round(diffSec / 60)
  const diffHour = Math.round(diffMin / 60)
  const diffDay = Math.round(diffHour / 24)
  const diffMonth = Math.round(diffDay / 30)
  const diffYear = Math.round(diffDay / 365)

  const rtf = new Intl.RelativeTimeFormat(SITE.locale, { numeric: 'auto' })
  if (Math.abs(diffSec) < 60) return rtf.format(-diffSec, 'second')
  if (Math.abs(diffMin) < 60) return rtf.format(-diffMin, 'minute')
  if (Math.abs(diffHour) < 24) return rtf.format(-diffHour, 'hour')
  if (Math.abs(diffDay) < 30) return rtf.format(-diffDay, 'day')
  if (Math.abs(diffMonth) < 12) return rtf.format(-diffMonth, 'month')
  return rtf.format(-diffYear, 'year')
}

export function getHeadingMargin(depth: number): string {
  const margins: Record<number, string> = {
    3: 'ml-4',
    4: 'ml-8',
    5: 'ml-12',
    6: 'ml-16',
  }
  return margins[depth] || ''
}

export function trimTrailingSlash(path: string): string {
  const pathname = path.split(/[?#]/, 1)[0] || '/'
  if (pathname === '/') return pathname
  return pathname.replace(/\/+$/, '')
}

export function isCurrentPath(href: string, pathname: string): boolean {
  return trimTrailingSlash(href) === trimTrailingSlash(pathname)
}

export function titleCase(input: string): string {
  return input
    .replace(/[-_]+/g, ' ')
    .split(' ')
    .filter(Boolean)
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ')
}
