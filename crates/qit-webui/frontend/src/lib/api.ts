import type {
  BlobResponse,
  BootstrapResponse,
  BranchesResponse,
  CommitHistory,
  CommitDetail,
  CommitsResponse,
  CompareResponse,
  PullRequestDetailResponse,
  PullRequestRecord,
  PullRequestsResponse,
  SettingsResponse,
  TreeResponse,
  UiRole,
} from './types'

const baseUrl =
  document.querySelector<HTMLMetaElement>('meta[name="qit-base"]')?.content ?? ''

async function requestJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${baseUrl}${path}`, {
    credentials: 'same-origin',
    headers: {
      'Content-Type': 'application/json',
      ...(init?.headers ?? {}),
    },
    ...init,
  })

  if (!response.ok) {
    const message = await response.text()
    throw new Error(message || `request failed with ${response.status}`)
  }

  if (response.status === 204) {
    return undefined as T
  }

  return (await response.json()) as T
}

export const api = {
  baseUrl,
  bootstrap: () => requestJson<BootstrapResponse>('/api/bootstrap'),
  login: async (username: string, password: string) => {
    const role = await requestJson<UiRole>('/api/session/login', {
      method: 'POST',
      body: JSON.stringify({ username, password }),
    })
    return role
  },
  logout: async () => {
    await requestJson<void>('/api/session/logout', { method: 'POST' })
  },
  settings: () => requestJson<SettingsResponse>('/api/settings'),
  updateSettings: (payload: { description?: string; homepage_url?: string }) =>
    requestJson<SettingsResponse>('/api/settings', {
      method: 'PATCH',
      body: JSON.stringify(payload),
    }),
  upsertBranchRule: (payload: {
    pattern: string
    require_pull_request: boolean
    required_approvals: number
    dismiss_stale_approvals: boolean
    block_force_push: boolean
    block_delete: boolean
  }) =>
    requestJson<SettingsResponse>('/api/settings/branch-rules', {
      method: 'PUT',
      body: JSON.stringify(payload),
    }),
  deleteBranchRule: (pattern: string) =>
    requestJson<SettingsResponse>(`/api/settings/branch-rules/${encodeURIComponent(pattern)}`, {
      method: 'DELETE',
    }),
  branches: async () => (await requestJson<BranchesResponse>('/api/branches')).branches,
  createBranch: (name: string, startPoint: string, force: boolean) =>
    requestJson('/api/branches', {
      method: 'POST',
      body: JSON.stringify({
        name,
        start_point: startPoint || null,
        force,
      }),
    }),
  checkoutBranch: (name: string, force: boolean) =>
    requestJson('/api/branches/checkout', {
      method: 'POST',
      body: JSON.stringify({ name, force }),
    }),
  switchBranch: (name: string) =>
    requestJson('/api/branches/switch', {
      method: 'POST',
      body: JSON.stringify({ name, force: false }),
    }),
  deleteBranch: (name: string) =>
    requestJson<void>(`/api/branches/${encodeURIComponent(name)}`, {
      method: 'DELETE',
    }),
  commits: (reference?: string, offset?: number, limit?: number) => {
    const query = new URLSearchParams()
    if (reference) query.set('reference', reference)
    if (typeof offset === 'number') query.set('offset', String(offset))
    if (typeof limit === 'number') query.set('limit', String(limit))
    return requestJson<CommitsResponse>(`/api/commits?${query.toString()}`).then(
      (response): CommitHistory => response.history,
    )
  },
  commit: (commit: string) =>
    requestJson<CommitDetail>(`/api/commits/${encodeURIComponent(commit)}`),
  tree: (reference?: string, path?: string) => {
    const query = new URLSearchParams()
    if (reference) query.set('reference', reference)
    if (path) query.set('path', path)
    return requestJson<TreeResponse>(`/api/code/tree?${query.toString()}`).then(
      (response) => response.entries,
    )
  },
  blob: (reference: string | undefined, path: string) => {
    const query = new URLSearchParams({ path })
    if (reference) query.set('reference', reference)
    return requestJson<BlobResponse>(`/api/code/blob?${query.toString()}`).then(
      (response) => response.blob,
    )
  },
  rawBlobUrl: (reference: string | undefined, path: string) => {
    const query = new URLSearchParams({ path })
    if (reference) query.set('reference', reference)
    return `${baseUrl}/api/code/raw?${query.toString()}`
  },
  compare: (base: string, head: string) =>
    requestJson<CompareResponse>(
      `/api/compare?base=${encodeURIComponent(base)}&head=${encodeURIComponent(head)}`,
    ).then((response) => response.comparison),
  pullRequests: async () =>
    (await requestJson<PullRequestsResponse>('/api/pull-requests')).pull_requests,
  pullRequest: (id: string) =>
    requestJson<PullRequestDetailResponse>(`/api/pull-requests/${encodeURIComponent(id)}`),
  createPullRequest: (payload: {
    title: string
    description: string
    source_branch: string
    target_branch: string
  }) =>
    requestJson<PullRequestRecord>('/api/pull-requests', {
      method: 'POST',
      body: JSON.stringify(payload),
    }),
  mergePullRequest: (id: string) =>
    requestJson<PullRequestRecord>(`/api/pull-requests/${encodeURIComponent(id)}/merge`, {
      method: 'POST',
    }),
  updatePullRequest: (
    id: string,
    payload: {
      title?: string
      description?: string
      status?: 'open' | 'closed'
    },
  ) =>
    requestJson<PullRequestRecord>(`/api/pull-requests/${encodeURIComponent(id)}`, {
      method: 'PATCH',
      body: JSON.stringify(payload),
    }),
  deletePullRequest: (id: string) =>
    requestJson<PullRequestRecord>(`/api/pull-requests/${encodeURIComponent(id)}`, {
      method: 'DELETE',
    }),
  commentPullRequest: (id: string, payload: { display_name: string; body: string }) =>
    requestJson<PullRequestRecord>(`/api/pull-requests/${encodeURIComponent(id)}/comments`, {
      method: 'POST',
      body: JSON.stringify(payload),
    }),
  reviewPullRequest: (
    id: string,
    payload: {
      display_name: string
      body: string
      state: 'commented' | 'approved' | 'changes_requested'
    },
  ) =>
    requestJson<PullRequestRecord>(`/api/pull-requests/${encodeURIComponent(id)}/reviews`, {
      method: 'POST',
      body: JSON.stringify(payload),
    }),
}
