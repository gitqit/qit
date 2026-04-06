import type { CommitRefDecoration } from '../../../lib/types'

export function shortSha(value: string) {
  return value.slice(0, 7)
}

export function formatTimestamp(value: number) {
  return new Date(value * 1000).toLocaleString()
}

export function formatRelativeTime(value: number) {
  const seconds = value - Math.floor(Date.now() / 1000)
  const formatter = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' })
  const ranges: Array<[Intl.RelativeTimeFormatUnit, number]> = [
    ['year', 60 * 60 * 24 * 365],
    ['month', 60 * 60 * 24 * 30],
    ['week', 60 * 60 * 24 * 7],
    ['day', 60 * 60 * 24],
    ['hour', 60 * 60],
    ['minute', 60],
  ]

  for (const [unit, divisor] of ranges) {
    if (Math.abs(seconds) >= divisor) {
      return formatter.format(Math.round(seconds / divisor), unit)
    }
  }

  return formatter.format(seconds, 'second')
}

export function formatDayHeading(value: number) {
  return new Date(value * 1000).toLocaleDateString(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  })
}

export function toneForRef(ref: CommitRefDecoration): 'accent' | 'success' | 'muted' {
  if (ref.is_current) return 'accent'
  if (ref.is_served) return 'success'
  return 'muted'
}

export function sortTreeEntries<T extends { kind: string; name: string }>(entries: T[]) {
  return [...entries].sort((left, right) => {
    if (left.kind !== right.kind) {
      return left.kind === 'tree' ? -1 : 1
    }
    return left.name.localeCompare(right.name, undefined, { sensitivity: 'base' })
  })
}

export function parentPath(path: string) {
  const slash = path.lastIndexOf('/')
  return slash === -1 ? '' : path.slice(0, slash)
}

export function formatBytes(size: number | null) {
  if (size == null) return 'Directory'
  if (size < 1024) return `${size} B`

  const units = ['KB', 'MB', 'GB']
  let value = size / 1024
  let unitIndex = 0

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024
    unitIndex += 1
  }

  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unitIndex]}`
}

export function isMarkdownPath(path: string) {
  return /\.(md|markdown|mdown|mkdn|mkd)$/i.test(path)
}
