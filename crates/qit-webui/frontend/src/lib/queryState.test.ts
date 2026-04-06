import { describe, expect, it } from 'vitest'
import { getQueryParam, mergeQueryParams } from './queryState'

describe('queryState', () => {
  it('reads known query params', () => {
    expect(getQueryParam('?tab=code&branch=feature%2Fdocs&commit=abc123', 'tab')).toBe('code')
    expect(getQueryParam('?tab=code&branch=feature%2Fdocs&commit=abc123', 'branch')).toBe('feature/docs')
    expect(getQueryParam('?tab=code&branch=feature%2Fdocs&commit=abc123', 'commit')).toBe('abc123')
    expect(getQueryParam('?tab=code&branch=feature%2Fdocs&commit=abc123', 'pr')).toBeNull()
  })

  it('merges updates without losing unrelated params', () => {
    expect(
      mergeQueryParams('/repo', '?tab=code&preview=1', {
        branch: 'feature/docs',
        tab: 'code',
      }),
    ).toBe('/repo?tab=code&preview=1&branch=feature%2Fdocs')
  })

  it('removes keys when values are cleared', () => {
    expect(
      mergeQueryParams('/repo', '?tab=code&branch=feature%2Fdocs&commit=abc123&pr=7', {
        branch: null,
        commit: null,
        pr: null,
      }),
    ).toBe('/repo?tab=code')
  })
})
