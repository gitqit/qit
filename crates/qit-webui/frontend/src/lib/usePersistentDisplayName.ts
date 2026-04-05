import { useEffect, useState } from 'react'

const STORAGE_KEY = 'qit.pull_request.display_name'

export function usePersistentDisplayName() {
  const [displayName, setDisplayName] = useState(() => {
    if (typeof window === 'undefined') {
      return ''
    }
    return window.localStorage.getItem(STORAGE_KEY) ?? ''
  })

  useEffect(() => {
    if (typeof window === 'undefined') {
      return
    }
    window.localStorage.setItem(STORAGE_KEY, displayName)
  }, [displayName])

  return [displayName, setDisplayName] as const
}
