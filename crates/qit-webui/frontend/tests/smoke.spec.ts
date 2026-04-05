import { expect, test } from '@playwright/test'

function mockApi(page: import('@playwright/test').Page, options: { actor: 'owner' | 'user' | null }) {
  let actor = options.actor

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
          repo_name: 'demo-repo',
          worktree: '/tmp/demo-repo',
          exported_branch: 'main',
          checked_out_branch: 'main',
          description: 'Hosted with Qit',
          homepage_url: 'https://example.com',
          local_only_owner_mode: actor === 'owner',
          shared_remote_identity: true,
          git_credentials_visible: actor === 'owner',
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

    if (pathname.endsWith('/api/settings')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          local_only_owner_mode: actor === 'owner',
          shared_remote_identity: true,
          repository: {
            description: 'Hosted with Qit',
            homepage_url: 'https://example.com',
            branch_rules: [],
          },
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
  await mockApi(page, { actor: null })
  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'Sign in to this session' })).toBeVisible()
  await page.getByLabel('Username').fill('session-owner')
  await page.getByLabel('Password').fill('temporary-pass')
  await page.getByRole('button', { name: 'Start session' }).click()

  await expect(page.getByRole('heading', { name: 'demo-repo' })).toBeVisible()
  await expect(page.getByText('Shared session')).toBeVisible()
})

test('renders the dashboard and owner clone details', async ({ page }) => {
  await mockApi(page, { actor: 'owner' })
  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'demo-repo' })).toBeVisible()
  await page.getByRole('button', { name: /clone/i }).click()
  await expect(page.locator('input[value="session-owner"]').first()).toBeVisible()
  await expect(page.getByText('Served branch: main')).toBeVisible()
})

test('clicking a branch opens the code view for that branch state', async ({ page }) => {
  await mockApi(page, { actor: 'owner' })
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
  await mockApi(page, { actor: 'owner' })
  await page.goto('/?tab=code&branch=missing')

  await expect(page).not.toHaveURL(/branch=missing/)
  await expect(page.getByRole('button', { name: /^README\.md/ })).toBeVisible()
  await expect(page.getByText('Initial commit')).toBeVisible()
})
