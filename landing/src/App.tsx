import { useEffect } from 'react'
import { BrowserRouter, Navigate, Route, Routes, useLocation } from 'react-router-dom'
import { DocsEmptyState, DocsPage } from './components/pages/DocsPage'
import { LandingPage } from './components/pages/LandingPage'
import { NotFoundPage } from './components/pages/NotFoundPage'
import { defaultDoc, getDocBySlug } from './lib/docs'

function DocsRoute() {
  const location = useLocation()
  const slug = location.pathname.replace(/^\/docs\/?/, '').replace(/\/+$/, '')

  if (!defaultDoc) {
    return <DocsEmptyState />
  }

  if (!slug) {
    return <Navigate replace to={defaultDoc.href} />
  }

  const doc = getDocBySlug(slug)
  if (!doc) {
    return <NotFoundPage description="That documentation page was not found." title="Doc not found" />
  }

  return <DocsPage doc={doc} />
}

function MetadataManager() {
  const location = useLocation()

  useEffect(() => {
    const slug = location.pathname.replace(/^\/docs\/?/, '').replace(/\/+$/, '')
    const fallbackSlug = defaultDoc?.slug ?? ''
    const doc = location.pathname.startsWith('/docs') ? getDocBySlug(slug || fallbackSlug) : null
    const title = doc ? `${doc.title} | Qit Docs` : location.pathname === '/' ? 'Qit | Quick Git for normal folders' : 'Qit'
    const description = doc
      ? doc.description
      : 'Install Qit, point it at any folder, and share it over authenticated Git Smart HTTP without reshaping your working tree.'

    document.title = title

    const descriptionMeta = document.querySelector('meta[name="description"]')
    if (descriptionMeta) {
      descriptionMeta.setAttribute('content', description)
    }
  }, [location.pathname])

  return null
}

function AppRoutes() {
  return (
    <>
      <MetadataManager />
      <Routes>
        <Route element={<LandingPage />} path="/" />
        <Route element={<DocsRoute />} path="/docs/*" />
        <Route element={<NotFoundPage />} path="*" />
      </Routes>
    </>
  )
}

function App() {
  return (
    <BrowserRouter basename={import.meta.env.BASE_URL}>
      <AppRoutes />
    </BrowserRouter>
  )
}

export default App
