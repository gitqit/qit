import { expect, test } from '@playwright/test'

function mockApi(
  page: import('@playwright/test').Page,
  options: { actor: 'owner' | 'user' | null; authMode?: 'shared_session' | 'request_based' },
) {
  let actor = options.actor
  const authMode = options.authMode ?? 'shared_session'
  const authMethods =
    authMode === 'shared_session' ? ['basic_auth'] : ['request_access', 'setup_token']

  const branches = [
    {
      name: 'main',
      is_current: true,
      is_served: true,
      commit: 'abc123',
      summary: 'Initial commit',
    },
    {
      name: 'feature/docs',
      is_current: false,
      is_served: false,
      commit: 'def456',
      summary: 'Docs branch commit',
    },
  ]

  const commitHistoryByReference = {
    main: {
      reference: 'main',
      offset: 0,
      limit: 40,
      has_more: false,
      commits: [
        {
          id: 'abc123',
          summary: 'Initial commit',
          author: 'Qit',
          authored_at: Date.now(),
          parents: [],
          refs: [],
        },
      ],
    },
    'feature/docs': {
      reference: 'feature/docs',
      offset: 0,
      limit: 40,
      has_more: false,
      commits: [
        {
          id: 'def456',
          summary: 'Docs branch commit',
          author: 'Qit',
          authored_at: Date.now(),
          parents: ['abc123'],
          refs: [],
        },
      ],
    },
  } as const

  const treeEntriesByReference = {
    main: [{ name: 'README.md', path: 'README.md', oid: '1', kind: 'blob', size: 12 }],
    'feature/docs': [{ name: 'docs.md', path: 'docs.md', oid: '2', kind: 'blob', size: 18 }],
  } as const

  return page.route('**/api/**', async (route) => {
    const url = new URL(route.request().url())
    const { pathname } = url
    const method = route.request().method()

    if (pathname.endsWith('/api/bootstrap')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          actor,
          principal:
            actor === 'user'
              ? {
                  user_id: 'user-1',
                  name: 'Alice',
                  email: 'alice@example.com',
                  username: 'alice',
                  role: 'user',
                }
              : null,
          repo_name: 'demo-repo',
          worktree: '/tmp/demo-repo',
          exported_branch: 'main',
          checked_out_branch: 'main',
          description: 'Hosted with Qit',
          homepage_url: 'https://example.com',
          auth_mode: authMode,
          auth_methods: authMethods,
          operator_override: actor === 'owner',
          local_only_owner_mode: actor === 'owner',
          shared_remote_identity: authMode === 'shared_session',
          git_credentials_visible: actor === 'owner' && authMode === 'shared_session',
          git_username: actor === 'owner' ? 'session-owner' : null,
          git_password: actor === 'owner' ? 'temporary-pass' : null,
          public_repo_url: 'https://demo.example/repo',
        }),
      })
      return
    }

    if (pathname.endsWith('/api/session/login') && method === 'POST') {
      actor = 'user'
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify('user'),
      })
      return
    }

    if (pathname.endsWith('/api/session/logout') && method === 'POST') {
      actor = null
      await route.fulfill({ status: 204, body: '' })
      return
    }

    if (pathname.endsWith('/api/onboarding/complete') && method === 'POST') {
      actor = 'user'
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify('user'),
      })
      return
    }

    if (pathname.endsWith('/api/access-requests') && method === 'POST') {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          request: {
            id: 'request-1',
            name: 'Alice',
            email: 'alice@example.com',
            status: 'pending',
            created_at_ms: Date.now(),
            reviewed_at_ms: null,
          },
          secret: 'qit_request.test.secret',
        }),
      })
      return
    }

    if (pathname.endsWith('/api/access-requests/status') && method === 'POST') {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          id: 'request-1',
          status: 'pending',
        }),
      })
      return
    }

    if (pathname.includes('/api/access-requests/') && pathname.endsWith('/approve') && method === 'POST') {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          user_id: 'user-1',
          email: 'alice@example.com',
          expires_at_ms: 0,
        }),
      })
      return
    }

    if (pathname.includes('/api/access-requests/') && pathname.endsWith('/reject') && method === 'POST') {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          id: 'request-1',
          name: 'Alice',
          email: 'alice@example.com',
          status: 'rejected',
          created_at_ms: Date.now(),
          reviewed_at_ms: Date.now(),
        }),
      })
      return
    }

    if (pathname.endsWith('/api/auth/mode') && method === 'POST') {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          auth_mode: authMode,
          auth_methods: authMethods,
          local_only_owner_mode: actor === 'owner',
          shared_remote_identity: authMode === 'shared_session',
          current_user:
            actor === 'user'
              ? {
                  user_id: 'user-1',
                  name: 'Alice',
                  email: 'alice@example.com',
                  username: 'alice',
                  role: 'user',
                }
              : null,
          users: [],
          access_requests: [],
          personal_access_tokens: [],
          repository: {
            description: 'Hosted with Qit',
            homepage_url: 'https://example.com',
            branch_rules: [],
          },
        }),
      })
      return
    }

    if (pathname.endsWith('/api/settings')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          auth_mode: authMode,
          auth_methods: authMethods,
          local_only_owner_mode: actor === 'owner',
          shared_remote_identity: authMode === 'shared_session',
          current_user:
            actor === 'user'
              ? {
                  user_id: 'user-1',
                  name: 'Alice',
                  email: 'alice@example.com',
                  username: 'alice',
                  role: 'user',
                }
              : null,
          users: actor === 'owner' && authMode === 'request_based'
            ? [
                {
                  id: 'user-1',
                  name: 'Alice',
                  email: 'alice@example.com',
                  username: 'alice',
                  role: 'user',
                  status: 'active',
                  created_at_ms: Date.now(),
                  approved_at_ms: Date.now(),
                  activated_at_ms: Date.now(),
                  revoked_at_ms: null,
                },
              ]
            : [],
          access_requests: actor === 'owner' && authMode === 'request_based'
            ? [
                {
                  id: 'request-1',
                  name: 'Alice',
                  email: 'alice@example.com',
                  status: 'pending',
                  created_at_ms: Date.now(),
                  reviewed_at_ms: null,
                },
              ]
            : [],
          personal_access_tokens:
            actor === 'user' && authMode === 'request_based'
              ? [{ id: 'pat-1', label: 'laptop', created_at_ms: Date.now(), revoked_at_ms: null }]
              : [],
          repository: {
            description: 'Hosted with Qit',
            homepage_url: 'https://example.com',
            branch_rules: [],
          },
        }),
      })
      return
    }

    if (pathname.endsWith('/api/profile/pats') && method === 'POST') {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          id: 'pat-2',
          label: 'new token',
          secret: 'qit_pat.test.secret',
          created_at_ms: Date.now(),
        }),
      })
      return
    }

    if (pathname.includes('/api/users/') && method === 'POST') {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          id: 'user-1',
          name: 'Alice',
          email: 'alice@example.com',
          username: 'alice',
          role: 'owner',
          status: 'active',
          created_at_ms: Date.now(),
          approved_at_ms: Date.now(),
          activated_at_ms: Date.now(),
          revoked_at_ms: null,
        }),
      })
      return
    }

    if (pathname.includes('/api/profile/pats/') && method === 'DELETE') {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          id: 'pat-1',
          label: 'laptop',
          created_at_ms: Date.now(),
          revoked_at_ms: Date.now(),
        }),
      })
      return
    }

    if (pathname.endsWith('/api/branches')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          branches,
        }),
      })
      return
    }

    if (pathname.endsWith('/api/commits')) {
      const reference = url.searchParams.get('reference') ?? 'main'
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          history: commitHistoryByReference[reference as keyof typeof commitHistoryByReference] ?? commitHistoryByReference.main,
        }),
      })
      return
    }

    if (pathname.endsWith('/api/commits/abc123')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          id: 'abc123',
          summary: 'Initial commit',
          message: 'Initial commit',
          author: 'Qit',
          authored_at: Date.now(),
          parents: [],
          changes: [],
        }),
      })
      return
    }

    if (pathname.endsWith('/api/code/tree')) {
      const reference = url.searchParams.get('reference') ?? 'main'
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          entries: treeEntriesByReference[reference as keyof typeof treeEntriesByReference] ?? treeEntriesByReference.main,
        }),
      })
      return
    }

    if (pathname.endsWith('/api/code/blob')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          blob: {
            path: 'README.md',
            text: '# Demo',
            is_binary: false,
            size: 6,
          },
        }),
      })
      return
    }

    if (pathname.endsWith('/api/pull-requests')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          pull_requests: [],
        }),
      })
      return
    }

    await route.fulfill({ status: 404, body: 'not mocked' })
  })
}

test('renders the login flow for shared sessions', async ({ page }) => {
  await mockApi(page, { actor: null, authMode: 'shared_session' })
  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'Access this repository' })).toBeVisible()
  await page.getByLabel('Username').fill('session-owner')
  await page.getByLabel('Password').fill('temporary-pass')
  await page.getByRole('button', { name: 'Start session' }).click()

  await expect(page.getByRole('heading', { name: 'demo-repo' })).toBeVisible()
  await expect(page.getByRole('button', { name: /clone/i })).toBeVisible()
})

test('signed-in repo users can open the account menu and log out', async ({ page }) => {
  await mockApi(page, { actor: 'user', authMode: 'request_based' })
  await page.goto('/')

  await page.getByRole('button', { name: 'alice@example.com' }).click()
  await expect(page.getByRole('menuitem', { name: 'User settings' })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: 'Log out' })).toBeVisible()
  await expect(page.getByText('Repo settings', { exact: true })).toBeVisible()

  await page.getByRole('menuitem', { name: 'User settings' }).click()
  await expect(page.getByRole('heading', { name: 'User settings' })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Personal access tokens' })).toBeVisible()

  await page.getByRole('button', { name: 'alice@example.com' }).click()
  await page.getByRole('menuitem', { name: 'Log out' }).click()
  await expect(page.getByRole('heading', { name: 'Access this repository' })).toBeVisible()
})

test('local operator account menu does not offer logout', async ({ page }) => {
  await mockApi(page, { actor: 'owner', authMode: 'request_based' })
  await page.goto('/')

  await page.getByRole('button', { name: 'Local operator' }).click()
  await expect(page.getByRole('menuitem', { name: 'User settings' })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: 'Log out' })).toHaveCount(0)

  await page.getByRole('menuitem', { name: 'User settings' }).click()
  await expect(page.getByRole('heading', { name: 'User settings' })).toBeVisible()
  await expect(page.getByText('No repo account')).toBeVisible()
})

test('renders the dashboard and owner clone details', async ({ page }) => {
  await mockApi(page, { actor: 'owner', authMode: 'shared_session' })
  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'demo-repo' })).toBeVisible()
  await page.getByRole('button', { name: /clone/i }).click()
  await expect(page.locator('input[value="session-owner"]').first()).toBeVisible()
  await expect(page.getByText('Served branch: main')).toBeVisible()
})

test('owner access alerts show pending requests with actions', async ({ page }) => {
  await mockApi(page, { actor: 'owner', authMode: 'request_based' })
  await page.goto('/')

  await page.getByLabel('1 pending alerts').click()
  await expect(page.getByText('Alerts')).toBeVisible()
  await expect(page.getByText('Alice', { exact: true })).toBeVisible()
  await expect(page.getByText('alice@example.com', { exact: true })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: 'Approve' })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: 'Reject' })).toBeVisible()

  await page.getByRole('menuitem', { name: 'Approve' }).click()
  await expect(
    page.getByText('Approved alice@example.com. They can finish signing in from their pending request browser tab.'),
  ).toBeVisible()
})

test('clicking a branch opens the code view for that branch state', async ({ page }) => {
  await mockApi(page, { actor: 'owner', authMode: 'shared_session' })
  await page.goto('/?tab=branches')

  await expect(page.getByRole('button', { name: 'Check out' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Serve' })).toHaveCount(0)
  await page.getByRole('button', { name: 'Branch actions for feature/docs' }).click()
  await expect(page.getByRole('menuitem', { name: 'Check out' })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: 'Serve' })).toBeVisible()
  await page.keyboard.press('Escape')

  await page.getByRole('button', { name: 'Browse code on feature/docs' }).click()

  await expect(page).toHaveURL(/tab=code/)
  await expect(page).toHaveURL(/branch=feature%2Fdocs/)
  await expect(page.getByText('Browsing branch: feature/docs')).toBeVisible()
  await expect(page.getByText('docs.md')).toBeVisible()
  await expect(page.getByText('Docs branch commit')).toBeVisible()
})

test('invalid branch query state falls back to the checked out branch', async ({ page }) => {
  await mockApi(page, { actor: 'owner', authMode: 'shared_session' })
  await page.goto('/?tab=code&branch=missing')

  await expect(page).not.toHaveURL(/branch=missing/)
  await expect(page.getByRole('button', { name: /^README\.md/ })).toBeVisible()
  await expect(page.getByText('Initial commit')).toBeVisible()
})

test('renders request-based auth actions', async ({ page }) => {
  await mockApi(page, { actor: null, authMode: 'request_based' })
  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'Access this repository' })).toBeVisible()
  await expect(page.getByLabel('Username')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Start session' })).toBeVisible()

  await page.getByRole('button', { name: 'Request access', exact: true }).click()
  await expect(page.getByLabel('Name')).toBeVisible()
  await expect(page.getByLabel('Email')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Send request' })).toBeVisible()

  await page.getByRole('button', { name: 'Complete setup', exact: true }).click()
  await expect(page.getByLabel('Onboarding token')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Finish setup' })).toBeVisible()
})

test('approved request moves setup forward in the same browser', async ({ page }) => {
  let statusChecks = 0

  await mockApi(page, { actor: null, authMode: 'request_based' })
  await page.route('**/api/access-requests/status', async (route) => {
    if (route.request().method() === 'POST') {
      statusChecks += 1
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          id: 'request-1',
          status: statusChecks >= 2 ? 'approved' : 'pending',
        }),
      })
      return
    }
    await route.fallback()
  })

  await page.goto('/')
  await page.getByRole('button', { name: 'Request access', exact: true }).click()
  await page.getByLabel('Name').fill('Alice')
  await page.getByLabel('Email').fill('alice@example.com')
  await page.getByRole('button', { name: 'Send request' }).click()

  await expect(page.getByText('Access request sent. Waiting for the owner to approve…')).toBeVisible()
  await expect(page.getByRole('heading', { name: 'demo-repo' })).toBeVisible({ timeout: 15_000 })
  expect(statusChecks).toBeGreaterThanOrEqual(2)
  await expect(page.getByLabel('Onboarding token')).toHaveCount(0)
})
