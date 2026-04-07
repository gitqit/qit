import { useEffect } from 'react'
import { BrowserRouter, Navigate, Route, Routes, useLocation } from 'react-router-dom'
import { DocsEmptyState, DocsPage } from './components/pages/DocsPage'
import { LandingPage } from './components/pages/LandingPage'
import { NotFoundPage } from './components/pages/NotFoundPage'
import { defaultDoc, getDocBySlug } from './lib/docs'

const siteTitle = 'Qit | Quick Git for normal folders'
const siteDescription = 'Install Qit, point it at any folder, and share it over authenticated Git Smart HTTP without reshaping your working tree.'
const socialImageAlt = 'Qit social card with the headline Quick Git for normal folders, a short product description, and gitqit.net.'

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

function upsertMeta(attribute: 'name' | 'property', key: string, content: string) {
  let meta = document.querySelector<HTMLMetaElement>(`meta[${attribute}="${key}"]`)

  if (!meta) {
    meta = document.createElement('meta')
    meta.setAttribute(attribute, key)
    document.head.appendChild(meta)
  }

  meta.setAttribute('content', content)
}

function upsertLink(rel: string, href: string) {
  let link = document.querySelector<HTMLLinkElement>(`link[rel="${rel}"]`)

  if (!link) {
    link = document.createElement('link')
    link.setAttribute('rel', rel)
    document.head.appendChild(link)
  }

  link.setAttribute('href', href)
}

function MetadataManager() {
  const location = useLocation()

  useEffect(() => {
    const slug = location.pathname.replace(/^\/docs\/?/, '').replace(/\/+$/, '')
    const fallbackSlug = defaultDoc?.slug ?? ''
    const doc = location.pathname.startsWith('/docs') ? getDocBySlug(slug || fallbackSlug) : null
    const title = doc ? `${doc.title} | Qit Docs` : location.pathname === '/' ? siteTitle : 'Qit'
    const description = doc ? doc.description : siteDescription
    const baseUrl = new URL(import.meta.env.BASE_URL, window.location.origin)
    const socialImageUrl = new URL('social-card.png', baseUrl).toString()
    const pageUrl = new URL(location.pathname, window.location.origin).toString()

    document.title = title

    upsertMeta('name', 'description', description)
    upsertMeta('property', 'og:title', title)
    upsertMeta('property', 'og:description', description)
    upsertMeta('property', 'og:type', 'website')
    upsertMeta('property', 'og:site_name', 'Qit')
    upsertMeta('property', 'og:url', pageUrl)
    upsertMeta('property', 'og:image', socialImageUrl)
    upsertMeta('property', 'og:image:type', 'image/png')
    upsertMeta('property', 'og:image:width', '1200')
    upsertMeta('property', 'og:image:height', '630')
    upsertMeta('property', 'og:image:alt', socialImageAlt)
    upsertMeta('name', 'twitter:card', 'summary_large_image')
    upsertMeta('name', 'twitter:title', title)
    upsertMeta('name', 'twitter:description', description)
    upsertMeta('name', 'twitter:image', socialImageUrl)
    upsertMeta('name', 'twitter:image:alt', socialImageAlt)
    upsertLink('canonical', pageUrl)
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
