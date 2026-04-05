import { useCallback, useEffect, useMemo, useState } from 'react'
import { api } from './lib/api'
import type {
  BlobContent,
  BootstrapResponse,
  BranchInfo,
  CommitDetail,
  CommitHistory,
  PullRequestRecord,
  SettingsResponse,
  TreeEntry,
} from './lib/types'
import { DashboardPage, LoginPage } from './components/pages/DashboardPage'
import { PreviewPage } from './components/pages/PreviewPage'

const HISTORY_PAGE_SIZE = 40

function isReadmeEntry(entry: TreeEntry) {
  return entry.kind === 'blob' && /^readme(?:\.[a-z0-9]+)?$/i.test(entry.name)
}

function parentPath(path: string) {
  const slash = path.lastIndexOf('/')
  return slash === -1 ? '' : path.slice(0, slash)
}

function App() {
  const initialParams = useMemo(() => new URLSearchParams(window.location.search), [])
  const previewMode = initialParams.get('preview')
  const [bootstrap, setBootstrap] = useState<BootstrapResponse | null>(null)
  const [settings, setSettings] = useState<SettingsResponse | null>(null)
  const [branches, setBranches] = useState<BranchInfo[]>([])
  const [history, setHistory] = useState<CommitHistory | null>(null)
  const [commitDetail, setCommitDetail] = useState<CommitDetail | null>(null)
  const [selectedCommitId, setSelectedCommitId] = useState<string | null>(initialParams.get('commit'))
  const [treeCache, setTreeCache] = useState<Record<string, TreeEntry[]>>({})
  const [readme, setReadme] = useState<BlobContent | null>(null)
  const [codePath, setCodePath] = useState('')
  const [codeTree, setCodeTree] = useState<TreeEntry[]>([])
  const [blob, setBlob] = useState<BlobContent | null>(null)
  const [pullRequests, setPullRequests] = useState<PullRequestRecord[]>([])
  const [selectedPullRequestId, setSelectedPullRequestId] = useState<string | null>(initialParams.get('pr'))
  const [loading, setLoading] = useState(true)
  const [loadingMoreCommits, setLoadingMoreCommits] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [authError, setAuthError] = useState<string | null>(null)
  const [toastMessage, setToastMessage] = useState<string | null>(null)
  const [highlightedPullRequestId, setHighlightedPullRequestId] = useState<string | null>(null)

  const actor = bootstrap?.actor ?? null

  const syncUrlState = useCallback((updates: Record<string, string | null>) => {
    const params = new URLSearchParams(window.location.search)
    for (const [key, value] of Object.entries(updates)) {
      if (value) {
        params.set(key, value)
      } else {
        params.delete(key)
      }
    }
    const query = params.toString()
    window.history.replaceState(null, '', query ? `${window.location.pathname}?${query}` : window.location.pathname)
  }, [])

  const loadAppState = useCallback(async () => {
    const nextBootstrap = await api.bootstrap()
    setBootstrap(nextBootstrap)

    if (!nextBootstrap.actor) {
      setSettings(null)
      setBranches([])
      setHistory(null)
      setCommitDetail(null)
      setSelectedCommitId(null)
      setTreeCache({})
      setReadme(null)
      setCodePath('')
      setCodeTree([])
      setBlob(null)
      setPullRequests([])
      setSelectedPullRequestId(null)
      return
    }

    const [nextSettings, nextBranches, nextHistory, nextRootTree, nextPullRequests] = await Promise.all([
      api.settings(),
      api.branches(),
      api.commits(nextBootstrap.checked_out_branch, 0, HISTORY_PAGE_SIZE),
      api.tree(nextBootstrap.checked_out_branch),
      api.pullRequests(),
    ])

    setSettings(nextSettings)
    setBranches(nextBranches)
    setHistory(nextHistory)
    setTreeCache({ '': nextRootTree })
    setCodePath('')
    setCodeTree(nextRootTree)
    setBlob(null)
    setPullRequests(nextPullRequests)

    const readmeEntry = nextRootTree.find(isReadmeEntry)
    if (readmeEntry) {
      setReadme(await api.blob(nextBootstrap.checked_out_branch, readmeEntry.path))
    } else {
      setReadme(null)
    }

    const nextSelectedPullRequestId =
      selectedPullRequestId && nextPullRequests.some((pullRequest) => pullRequest.id === selectedPullRequestId)
        ? selectedPullRequestId
        : null

    if (nextSelectedPullRequestId) {
      setSelectedPullRequestId(nextSelectedPullRequestId)
      setSelectedCommitId(null)
      setCommitDetail(null)
      return
    }

    setSelectedPullRequestId(null)

    const nextCommitId =
      selectedCommitId && nextHistory.commits.some((commit) => commit.id === selectedCommitId)
        ? selectedCommitId
        : nextHistory.commits[0]?.id ?? null

    if (nextCommitId) {
      setSelectedCommitId(nextCommitId)
      setCommitDetail(await api.commit(nextCommitId))
    } else {
      setSelectedCommitId(null)
      setCommitDetail(null)
    }
  }, [selectedCommitId, selectedPullRequestId])

  useEffect(() => {
    const run = async () => {
      setLoading(true)
      setError(null)
      try {
        await loadAppState()
      } catch (loadError) {
        setError(loadError instanceof Error ? loadError.message : 'failed to load UI state')
      } finally {
        setLoading(false)
      }
    }
    void run()
  }, [loadAppState])

  useEffect(() => {
    if (!toastMessage) {
      return
    }

    const timeoutId = window.setTimeout(() => {
      setToastMessage(null)
    }, 3500)

    return () => window.clearTimeout(timeoutId)
  }, [toastMessage])

  useEffect(() => {
    if (!highlightedPullRequestId) {
      return
    }

    const timeoutId = window.setTimeout(() => {
      setHighlightedPullRequestId(null)
    }, 5000)

    return () => window.clearTimeout(timeoutId)
  }, [highlightedPullRequestId])

  useEffect(() => {
    syncUrlState({
      commit: selectedCommitId,
      pr: selectedPullRequestId,
    })
  }, [selectedCommitId, selectedPullRequestId, syncUrlState])

  useEffect(() => {
    // #region agent log
    fetch('http://127.0.0.1:7706/ingest/50a22906-0850-4d7b-8a83-df8ca342990a', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-Debug-Session-Id': '675ac5',
      },
      body: JSON.stringify({
        sessionId: '675ac5',
        runId: 'post-fix',
        hypothesisId: 'C5',
        location: 'crates/qit-webui/frontend/src/App.tsx:171',
        message: 'Selection state changed',
        data: {
          selectedCommitId,
          selectedPullRequestId,
        },
        timestamp: Date.now(),
      }),
    }).catch(() => {})
    // #endregion
  }, [selectedCommitId, selectedPullRequestId])

  const refresh = useCallback(async () => {
    setError(null)
    await loadAppState()
  }, [loadAppState])

  const currentRef = useMemo(
    () => bootstrap?.checked_out_branch ?? bootstrap?.exported_branch,
    [bootstrap],
  )

  const loadTreePath = useCallback(
    async (path: string, force = false) => {
      if (!currentRef) {
        return []
      }

      if (!force) {
        const cachedEntries = treeCache[path]
        if (cachedEntries) {
          return cachedEntries
        }
      }

      const entries = await api.tree(currentRef, path || undefined)
      setTreeCache((previous) => ({ ...previous, [path]: entries }))
      return entries
    },
    [currentRef, treeCache],
  )

  const browseTree = useCallback(
    async (path: string) => {
      if (!currentRef) {
        return
      }

      const nextTree = await loadTreePath(path)
      const readmeEntry = nextTree.find(isReadmeEntry)
      setCodePath(path)
      setCodeTree(nextTree)
      setReadme(readmeEntry ? await api.blob(currentRef, readmeEntry.path) : null)
      setBlob(null)
    },
    [currentRef, loadTreePath],
  )

  const openBlob = useCallback(
    async (path: string) => {
      if (!currentRef) {
        return
      }

      const nextPath = parentPath(path)
      const [nextTree, nextBlob] = await Promise.all([
        loadTreePath(nextPath),
        api.blob(currentRef, path),
      ])
      setCodePath(nextPath)
      setCodeTree(nextTree)
      setBlob(nextBlob)
    },
    [currentRef, loadTreePath],
  )

  if (previewMode === 'ui') {
    return <PreviewPage />
  }

  if (loading && !bootstrap) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-canvas text-fg">
        <p className="text-sm text-fg-muted">Loading Qit…</p>
      </div>
    )
  }

  if (!bootstrap) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-canvas text-danger">
        <p>{error ?? 'Failed to load this Qit session.'}</p>
      </div>
    )
  }

  if (!actor) {
    return (
      <LoginPage
        error={authError}
        onLogin={async (username, password) => {
          setAuthError(null)
          try {
            await api.login(username, password)
            await refresh()
          } catch (loginError) {
            setAuthError(loginError instanceof Error ? loginError.message : 'Unable to start the session.')
          }
        }}
      />
    )
  }

  return (
    <div>
      {error ? (
        <div className="border-b border-danger/40 bg-danger/12 px-6 py-3 text-sm text-danger">
          {error}
        </div>
      ) : null}
      {toastMessage ? (
        <div className="fixed right-4 top-4 z-40 max-w-sm rounded-token border border-success/40 bg-success/12 px-4 py-3 text-sm text-success shadow-panel" role="status">
          {toastMessage}
        </div>
      ) : null}
      <DashboardPage
        activeBlob={blob}
        actor={actor}
        bootstrap={bootstrap}
        branches={branches}
        commitDetail={commitDetail}
        codePath={codePath}
        codeTree={codeTree}
        history={history}
        highlightedPullRequestId={highlightedPullRequestId}
        loadingMoreCommits={loadingMoreCommits}
        selectedPullRequestId={selectedPullRequestId}
        treeCache={treeCache}
        onCheckoutBranch={async (name) => {
          await api.checkoutBranch(name, false)
          await refresh()
        }}
        onCreateBranch={async (name, startPoint, force) => {
          await api.createBranch(name, startPoint, force)
          await refresh()
        }}
        onCreatePullRequest={async (payload) => {
          const createdPullRequest = await api.createPullRequest(payload)
          setPullRequests((current) => {
            const nextPullRequests = current.filter((pullRequest) => pullRequest.id !== createdPullRequest.id)
            return [createdPullRequest, ...nextPullRequests]
          })
          setSelectedPullRequestId(createdPullRequest.id)
          setHighlightedPullRequestId(createdPullRequest.id)
          setToastMessage(`Pull request "${createdPullRequest.title}" created.`)
          void refresh().catch((loadError) => {
            setError(loadError instanceof Error ? loadError.message : 'failed to refresh UI state')
          })
          return createdPullRequest
        }}
        onDeleteBranch={async (name) => {
          await api.deleteBranch(name)
          await refresh()
        }}
        onDeletePullRequest={async (id) => {
          const deletedPullRequest = await api.deletePullRequest(id)
          if (selectedPullRequestId === id) {
            setSelectedPullRequestId(null)
          }
          setToastMessage(`Pull request "${deletedPullRequest.title}" deleted.`)
          await refresh()
          return deletedPullRequest
        }}
        onMergePullRequest={async (id) => {
          await api.mergePullRequest(id)
          await refresh()
        }}
        onReviewPullRequest={async (id, payload) => {
          const updatedPullRequest = await api.reviewPullRequest(id, payload)
          await refresh()
          return updatedPullRequest
        }}
        onLoadMoreCommits={async () => {
          if (!currentRef || !history || !history.has_more) {
            return
          }

          setLoadingMoreCommits(true)
          try {
            const nextHistory = await api.commits(
              currentRef,
              history.offset + history.commits.length,
              history.limit,
            )
            setHistory((previous) =>
              previous
                ? {
                    ...nextHistory,
                    offset: previous.offset,
                    commits: [...previous.commits, ...nextHistory.commits],
                  }
                : nextHistory,
            )
          } finally {
            setLoadingMoreCommits(false)
          }
        }}
        onBrowseTree={async (path) => {
          await browseTree(path)
        }}
        onLoadTreePath={loadTreePath}
        onOpenTreeEntry={async (entry) => {
          if (entry.kind === 'tree') {
            await browseTree(entry.path)
            return
          }

          await openBlob(entry.path)
        }}
        onSelectCommit={async (commit) => {
          setSelectedPullRequestId(null)
          setSelectedCommitId(commit)
          setCommitDetail(await api.commit(commit))
        }}
        onSelectPullRequest={(id) => {
          setSelectedCommitId(null)
          setCommitDetail(null)
          setSelectedPullRequestId(id)
        }}
        onClearSelectedPullRequest={() => {
          setSelectedPullRequestId(null)
        }}
        onCommentPullRequest={async (id, payload) => {
          const updatedPullRequest = await api.commentPullRequest(id, payload)
          await refresh()
          return updatedPullRequest
        }}
        onSwitchBranch={async (name) => {
          await api.switchBranch(name)
          await refresh()
        }}
        onUpdatePullRequest={async (id, payload) => {
          const updatedPullRequest = await api.updatePullRequest(id, payload)
          await refresh()
          return updatedPullRequest
        }}
        pullRequests={pullRequests}
        readme={readme}
        selectedCommitId={selectedCommitId}
        settings={settings}
      />
    </div>
  )
}

export default App
