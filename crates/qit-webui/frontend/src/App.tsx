import { useCallback, useEffect, useMemo, useState } from 'react'
import { api } from './lib/api'
import { getQueryParam, mergeQueryParams } from './lib/queryState'
import type {
  BlobContent,
  BootstrapResponse,
  BranchInfo,
  CommitDetail,
  CommitHistory,
  PullRequestRecord,
  RefDiffFile,
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

async function loadReadme(reference: string, entries: TreeEntry[]) {
  const readmeEntry = entries.find(isReadmeEntry)
  return readmeEntry ? api.blob(reference, readmeEntry.path) : null
}

async function loadOptionalBlob(reference: string, path: string) {
  try {
    return await api.blob(reference, path)
  } catch {
    return null
  }
}

function App() {
  const previewMode = useMemo(() => getQueryParam(window.location.search, 'preview'), [])
  const [bootstrap, setBootstrap] = useState<BootstrapResponse | null>(null)
  const [settings, setSettings] = useState<SettingsResponse | null>(null)
  const [branches, setBranches] = useState<BranchInfo[]>([])
  const [history, setHistory] = useState<CommitHistory | null>(null)
  const [commitDetail, setCommitDetail] = useState<CommitDetail | null>(null)
  const [loadingCommitDetail, setLoadingCommitDetail] = useState(false)
  const [selectedCommitId, setSelectedCommitId] = useState<string | null>(() => getQueryParam(window.location.search, 'commit'))
  const [selectedBranch, setSelectedBranch] = useState<string | null>(() => getQueryParam(window.location.search, 'branch'))
  const [treeCache, setTreeCache] = useState<Record<string, TreeEntry[]>>({})
  const [readme, setReadme] = useState<BlobContent | null>(null)
  const [codePath, setCodePath] = useState('')
  const [codeTree, setCodeTree] = useState<TreeEntry[]>([])
  const [blob, setBlob] = useState<BlobContent | null>(null)
  const [commitTreeCache, setCommitTreeCache] = useState<Record<string, TreeEntry[]>>({})
  const [commitReadme, setCommitReadme] = useState<BlobContent | null>(null)
  const [commitCodePath, setCommitCodePath] = useState('')
  const [commitCodeTree, setCommitCodeTree] = useState<TreeEntry[]>([])
  const [commitActivePath, setCommitActivePath] = useState<string | null>(null)
  const [commitBlob, setCommitBlob] = useState<BlobContent | null>(null)
  const [commitDiff, setCommitDiff] = useState<RefDiffFile | null>(null)
  const [pullRequests, setPullRequests] = useState<PullRequestRecord[]>([])
  const [selectedPullRequestId, setSelectedPullRequestId] = useState<string | null>(() => getQueryParam(window.location.search, 'pr'))
  const [loading, setLoading] = useState(true)
  const [loadingMoreCommits, setLoadingMoreCommits] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [authError, setAuthError] = useState<string | null>(null)
  const [toastMessage, setToastMessage] = useState<string | null>(null)
  const [highlightedPullRequestId, setHighlightedPullRequestId] = useState<string | null>(null)

  const actor = bootstrap?.actor ?? null

  const syncUrlState = useCallback((updates: Record<string, string | null>) => {
    window.history.replaceState(
      null,
      '',
      mergeQueryParams(window.location.pathname, window.location.search, updates),
    )
  }, [])

  const pushUrlState = useCallback((updates: Record<string, string | null>) => {
    window.history.pushState(
      null,
      '',
      mergeQueryParams(window.location.pathname, window.location.search, updates),
    )
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
      setLoadingCommitDetail(false)
      setCommitTreeCache({})
      setCommitReadme(null)
      setCommitCodePath('')
      setCommitCodeTree([])
      setCommitActivePath(null)
      setCommitBlob(null)
      setCommitDiff(null)
      setPullRequests([])
      setSelectedPullRequestId(null)
      return
    }

    const [nextSettings, nextBranches, nextPullRequests] = await Promise.all([
      api.settings(),
      api.branches(),
      api.pullRequests(),
    ])

    setSettings(nextSettings)
    setBranches(nextBranches)
    setPullRequests(nextPullRequests)

    const nextSelectedPullRequestId =
      selectedPullRequestId && nextPullRequests.some((pullRequest) => pullRequest.id === selectedPullRequestId)
        ? selectedPullRequestId
        : null

    if (nextSelectedPullRequestId) {
      setSelectedPullRequestId(nextSelectedPullRequestId)
      setSelectedCommitId(null)
      setCommitDetail(null)
      setLoadingCommitDetail(false)
      setCommitTreeCache({})
      setCommitReadme(null)
      setCommitCodePath('')
      setCommitCodeTree([])
      setCommitActivePath(null)
      setCommitBlob(null)
      setCommitDiff(null)
      return
    }

    setSelectedPullRequestId(null)
  }, [selectedPullRequestId])

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
      branch: selectedBranch,
      commit: selectedCommitId,
      pr: selectedPullRequestId,
    })
  }, [selectedBranch, selectedCommitId, selectedPullRequestId, syncUrlState])

  useEffect(() => {
    const handlePopState = () => {
      setSelectedBranch(getQueryParam(window.location.search, 'branch'))
      setSelectedCommitId(getQueryParam(window.location.search, 'commit'))
      setSelectedPullRequestId(getQueryParam(window.location.search, 'pr'))
    }

    window.addEventListener('popstate', handlePopState)
    return () => window.removeEventListener('popstate', handlePopState)
  }, [])

  const refresh = useCallback(async () => {
    setError(null)
    await loadAppState()
  }, [loadAppState])

  const selectedBranchExists = useMemo(
    () => (selectedBranch ? branches.some((branch) => branch.name === selectedBranch) : false),
    [branches, selectedBranch],
  )

  const currentRef = useMemo(
    () => {
      if (!bootstrap) {
        return null
      }

      if (selectedBranch) {
        if (loading) {
          return null
        }

        return selectedBranchExists ? selectedBranch : bootstrap.checked_out_branch ?? bootstrap.exported_branch
      }

      return bootstrap.checked_out_branch ?? bootstrap.exported_branch
    },
    [bootstrap, loading, selectedBranch, selectedBranchExists],
  )

  useEffect(() => {
    if (!bootstrap || loading || !selectedBranch) {
      return
    }

    if (selectedBranch === bootstrap.checked_out_branch || !selectedBranchExists) {
      setSelectedBranch(null)
    }
  }, [bootstrap, loading, selectedBranch, selectedBranchExists])

  useEffect(() => {
    if (!actor || !currentRef) {
      setHistory(null)
      return
    }

    let cancelled = false

    const run = async () => {
      setHistory(null)
      try {
        const nextHistory = await api.commits(currentRef, 0, HISTORY_PAGE_SIZE)
        if (!cancelled) {
          setHistory(nextHistory)
        }
      } catch (loadError) {
        if (!cancelled) {
          setHistory(null)
          setError(loadError instanceof Error ? loadError.message : 'failed to load commit history')
        }
      }
    }

    void run()
    return () => {
      cancelled = true
    }
  }, [actor, currentRef])

  useEffect(() => {
    if (!actor || !currentRef) {
      setTreeCache({})
      setReadme(null)
      setCodePath('')
      setCodeTree([])
      setBlob(null)
      return
    }

    let cancelled = false

    const run = async () => {
      setTreeCache({})
      setReadme(null)
      setCodePath('')
      setCodeTree([])
      setBlob(null)

      try {
        const nextRootTree = await api.tree(currentRef)
        const nextReadme = await loadReadme(currentRef, nextRootTree)

        if (cancelled) {
          return
        }

        setTreeCache({ '': nextRootTree })
        setReadme(nextReadme)
        setCodePath('')
        setCodeTree(nextRootTree)
        setBlob(null)
      } catch (loadError) {
        if (!cancelled) {
          setTreeCache({})
          setReadme(null)
          setCodePath('')
          setCodeTree([])
          setBlob(null)
          setError(loadError instanceof Error ? loadError.message : 'failed to load code browser')
        }
      }
    }

    void run()
    return () => {
      cancelled = true
    }
  }, [actor, currentRef])

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

  const loadCommitTreePath = useCallback(
    async (path: string, force = false) => {
      if (!selectedCommitId) {
        return []
      }

      if (!force) {
        const cachedEntries = commitTreeCache[path]
        if (cachedEntries) {
          return cachedEntries
        }
      }

      const entries = await api.tree(selectedCommitId, path || undefined)
      setCommitTreeCache((previous) => ({ ...previous, [path]: entries }))
      return entries
    },
    [commitTreeCache, selectedCommitId],
  )

  const browseCommitTree = useCallback(
    async (path: string) => {
      if (!selectedCommitId) {
        return
      }

      const nextTree = await loadCommitTreePath(path)
      setCommitCodePath(path)
      setCommitCodeTree(nextTree)
      setCommitReadme(await loadReadme(selectedCommitId, nextTree))
      setCommitActivePath(null)
      setCommitBlob(null)
      setCommitDiff(null)
    },
    [loadCommitTreePath, selectedCommitId],
  )

  const openCommitBlob = useCallback(
    async (path: string, detailOverride?: CommitDetail | null) => {
      const commitRef = selectedCommitId
      if (!commitRef) {
        return
      }

      const detailForPath = detailOverride ?? commitDetail
      const nextPath = parentPath(path)
      const change = detailForPath?.changes.find((entry) => entry.path === path) ?? null
      const parentCommit = detailForPath?.parents[0] ?? null
      const [nextTree, nextBlob, originalBlob] = await Promise.all([
        loadCommitTreePath(nextPath, Boolean(detailOverride)),
        loadOptionalBlob(commitRef, path),
        change && parentCommit ? loadOptionalBlob(parentCommit, path) : Promise.resolve(null),
      ])
      setCommitCodePath(nextPath)
      setCommitCodeTree(nextTree)
      setCommitActivePath(path)
      setCommitBlob(nextBlob)
      setCommitDiff(
        change
          ? {
              path,
              previous_path: null,
              status: change.status,
              additions: change.additions,
              deletions: change.deletions,
              original: originalBlob,
              modified: nextBlob,
            }
          : null,
      )
    },
    [commitDetail, loadCommitTreePath, selectedCommitId],
  )

  useEffect(() => {
    if (!selectedCommitId) {
      setCommitDetail(null)
      setLoadingCommitDetail(false)
      setCommitTreeCache({})
      setCommitReadme(null)
      setCommitCodePath('')
      setCommitCodeTree([])
      setCommitActivePath(null)
      setCommitBlob(null)
      setCommitDiff(null)
      return
    }

    let cancelled = false

    const run = async () => {
      setLoadingCommitDetail(true)
      setCommitTreeCache({})
      setCommitReadme(null)
      setCommitCodePath('')
      setCommitCodeTree([])
      setCommitActivePath(null)
      setCommitBlob(null)
      setCommitDiff(null)
      try {
        const [nextDetail, nextRootTree] = await Promise.all([
          api.commit(selectedCommitId),
          api.tree(selectedCommitId),
        ])
        const nextReadme = await loadReadme(selectedCommitId, nextRootTree)

        if (cancelled) {
          return
        }

        setCommitDetail(nextDetail)
        setCommitTreeCache({ '': nextRootTree })
        setCommitReadme(nextReadme)
        setCommitCodePath('')
        setCommitCodeTree(nextRootTree)
        setCommitActivePath(null)
        setCommitBlob(null)
        setCommitDiff(null)
        const firstChangedFile = nextDetail.changes.find((change) => !change.status.toLowerCase().includes('rename from'))
        if (firstChangedFile) {
          const nextPath = parentPath(firstChangedFile.path)
          const parentCommit = nextDetail.parents[0] ?? null
          const [nextTree, nextBlob, originalBlob] = await Promise.all([
            nextPath ? api.tree(selectedCommitId, nextPath) : Promise.resolve(nextRootTree),
            loadOptionalBlob(selectedCommitId, firstChangedFile.path),
            parentCommit ? loadOptionalBlob(parentCommit, firstChangedFile.path) : Promise.resolve(null),
          ])

          if (cancelled) {
            return
          }

          setCommitTreeCache(nextPath ? { '': nextRootTree, [nextPath]: nextTree } : { '': nextRootTree })
          setCommitCodePath(nextPath)
          setCommitCodeTree(nextTree)
          setCommitActivePath(firstChangedFile.path)
          setCommitBlob(nextBlob)
          setCommitDiff({
            path: firstChangedFile.path,
            previous_path: null,
            status: firstChangedFile.status,
            additions: firstChangedFile.additions,
            deletions: firstChangedFile.deletions,
            original: originalBlob,
            modified: nextBlob,
          })
          return
        }
      } catch (loadError) {
        if (cancelled) {
          return
        }

        setCommitDetail(null)
        setCommitTreeCache({})
        setCommitReadme(null)
        setCommitCodePath('')
        setCommitCodeTree([])
        setCommitActivePath(null)
        setCommitBlob(null)
        setCommitDiff(null)
        setError(loadError instanceof Error ? loadError.message : 'failed to load commit snapshot')
      } finally {
        if (!cancelled) {
          setLoadingCommitDetail(false)
        }
      }
    }

    void run()
    return () => {
      cancelled = true
    }
  }, [selectedCommitId])

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
        commitSnapshotBlob={commitBlob}
        commitSnapshotDiff={commitDiff}
        commitSnapshotActivePath={commitActivePath}
        commitSnapshotPath={commitCodePath}
        commitSnapshotReadme={commitReadme}
        commitSnapshotTree={commitCodeTree}
        commitSnapshotTreeCache={commitTreeCache}
        codePath={codePath}
        codeTree={codeTree}
        history={history}
        highlightedPullRequestId={highlightedPullRequestId}
        loadingCommitDetail={loadingCommitDetail}
        loadingMoreCommits={loadingMoreCommits}
        selectedBranch={selectedBranchExists ? selectedBranch : null}
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
        onDeleteBranchRule={async (pattern) => {
          await api.deleteBranchRule(pattern)
          setToastMessage(`Branch rule "${pattern}" deleted.`)
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
        onBrowseCommitTree={async (path) => {
          await browseCommitTree(path)
        }}
        onLoadCommitTreePath={loadCommitTreePath}
        onOpenCommitTreeEntry={async (entry) => {
          if (entry.kind === 'tree') {
            await browseCommitTree(entry.path)
            return
          }

          await openCommitBlob(entry.path)
        }}
        onViewBranchCode={(name) => {
          const nextBranch = name === bootstrap.checked_out_branch ? null : name
          pushUrlState({ branch: nextBranch, commit: null, pr: null, tab: 'code' })
          setSelectedCommitId(null)
          setCommitDetail(null)
          setSelectedPullRequestId(null)
          setSelectedBranch(nextBranch)
        }}
        onSelectCommit={(commit) => {
          pushUrlState({ commit, pr: null })
          setSelectedPullRequestId(null)
          setSelectedCommitId(commit)
        }}
        onClearSelectedCommit={() => {
          pushUrlState({ commit: null })
          setSelectedCommitId(null)
        }}
        onSelectPullRequest={(id) => {
          pushUrlState({ commit: null, pr: id })
          setSelectedCommitId(null)
          setCommitDetail(null)
          setSelectedPullRequestId(id)
        }}
        onClearSelectedPullRequest={() => {
          pushUrlState({ pr: null })
          setSelectedPullRequestId(null)
        }}
        onCommentPullRequest={async (id, payload) => {
          const updatedPullRequest = await api.commentPullRequest(id, payload)
          await refresh()
          return updatedPullRequest
        }}
        onSwitchBranch={async (name) => {
          await api.switchBranch(name)
          setToastMessage(`Default branch switched to "${name}".`)
          await refresh()
        }}
        onUpdateBranchRule={async (payload) => {
          await api.upsertBranchRule(payload)
          setToastMessage(`Branch rule "${payload.pattern}" saved.`)
          await refresh()
        }}
        onUpdateSettings={async (payload) => {
          await api.updateSettings(payload)
          setToastMessage('Repository settings saved.')
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
