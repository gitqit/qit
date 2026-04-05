export function getQueryParam(search: string, key: string): string | null {
  return new URLSearchParams(search).get(key)
}

export function mergeQueryParams(
  pathname: string,
  search: string,
  updates: Record<string, string | null>,
): string {
  const params = new URLSearchParams(search)
  for (const [key, value] of Object.entries(updates)) {
    if (value) {
      params.set(key, value)
    } else {
      params.delete(key)
    }
  }
  const query = params.toString()
  return query ? `${pathname}?${query}` : pathname
}
