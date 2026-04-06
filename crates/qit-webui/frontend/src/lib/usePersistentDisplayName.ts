import { useEffect, useState } from 'react'

const DEFAULT_STORAGE_KEY = 'qit.pull_request.display_name'

export function usePersistentDisplayName(storageKey = DEFAULT_STORAGE_KEY) {
  const [displayName, setDisplayName] = useState(() => {
    if (typeof window === 'undefined') {
      return ''
    }
    return window.localStorage.getItem(storageKey) ?? ''
  })

  useEffect(() => {
    if (typeof window === 'undefined') {
      return
    }
    window.localStorage.setItem(storageKey, displayName)
  }, [displayName, storageKey])

  return [displayName, setDisplayName] as const
}
