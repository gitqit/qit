import { FileText, Folder, FolderOpen } from 'lucide-react'
import cssIconUrl from 'material-icon-theme/icons/css.svg?url'
import fileIconUrl from 'material-icon-theme/icons/file.svg?url'
import folderComponentsIconUrl from 'material-icon-theme/icons/folder-components.svg?url'
import folderComponentsOpenIconUrl from 'material-icon-theme/icons/folder-components-open.svg?url'
import folderDocsIconUrl from 'material-icon-theme/icons/folder-docs.svg?url'
import folderDocsOpenIconUrl from 'material-icon-theme/icons/folder-docs-open.svg?url'
import folderGitIconUrl from 'material-icon-theme/icons/folder-git.svg?url'
import folderGitOpenIconUrl from 'material-icon-theme/icons/folder-git-open.svg?url'
import folderIconUrl from 'material-icon-theme/icons/folder.svg?url'
import folderImagesIconUrl from 'material-icon-theme/icons/folder-images.svg?url'
import folderImagesOpenIconUrl from 'material-icon-theme/icons/folder-images-open.svg?url'
import folderLibIconUrl from 'material-icon-theme/icons/folder-lib.svg?url'
import folderLibOpenIconUrl from 'material-icon-theme/icons/folder-lib-open.svg?url'
import folderOpenIconUrl from 'material-icon-theme/icons/folder-open.svg?url'
import folderPackagesIconUrl from 'material-icon-theme/icons/folder-packages.svg?url'
import folderPackagesOpenIconUrl from 'material-icon-theme/icons/folder-packages-open.svg?url'
import folderPublicIconUrl from 'material-icon-theme/icons/folder-public.svg?url'
import folderPublicOpenIconUrl from 'material-icon-theme/icons/folder-public-open.svg?url'
import folderSrcIconUrl from 'material-icon-theme/icons/folder-src.svg?url'
import folderSrcOpenIconUrl from 'material-icon-theme/icons/folder-src-open.svg?url'
import folderTestIconUrl from 'material-icon-theme/icons/folder-test.svg?url'
import folderTestOpenIconUrl from 'material-icon-theme/icons/folder-test-open.svg?url'
import gitIconUrl from 'material-icon-theme/icons/git.svg?url'
import htmlIconUrl from 'material-icon-theme/icons/html.svg?url'
import imageIconUrl from 'material-icon-theme/icons/image.svg?url'
import javascriptIconUrl from 'material-icon-theme/icons/javascript.svg?url'
import jsonIconUrl from 'material-icon-theme/icons/json.svg?url'
import lockIconUrl from 'material-icon-theme/icons/lock.svg?url'
import markdownIconUrl from 'material-icon-theme/icons/markdown.svg?url'
import nodejsIconUrl from 'material-icon-theme/icons/nodejs.svg?url'
import reactIconUrl from 'material-icon-theme/icons/react.svg?url'
import reactTsIconUrl from 'material-icon-theme/icons/react_ts.svg?url'
import readmeIconUrl from 'material-icon-theme/icons/readme.svg?url'
import rustIconUrl from 'material-icon-theme/icons/rust.svg?url'
import svgIconUrl from 'material-icon-theme/icons/svg.svg?url'
import tomlIconUrl from 'material-icon-theme/icons/toml.svg?url'
import typescriptIconUrl from 'material-icon-theme/icons/typescript.svg?url'
import yamlIconUrl from 'material-icon-theme/icons/yaml.svg?url'

const fileNameIconMap: Record<string, string> = {
  '.gitignore': gitIconUrl,
  'cargo.lock': lockIconUrl,
  'cargo.toml': tomlIconUrl,
  'package-lock.json': nodejsIconUrl,
  'package.json': nodejsIconUrl,
  'readme.md': readmeIconUrl,
}

const fileExtensionIconMap: Record<string, string> = {
  css: cssIconUrl,
  gif: imageIconUrl,
  git: gitIconUrl,
  html: htmlIconUrl,
  jpeg: imageIconUrl,
  jpg: imageIconUrl,
  js: javascriptIconUrl,
  json: jsonIconUrl,
  lock: lockIconUrl,
  markdown: markdownIconUrl,
  md: markdownIconUrl,
  png: imageIconUrl,
  rs: rustIconUrl,
  svg: svgIconUrl,
  toml: tomlIconUrl,
  ts: typescriptIconUrl,
  tsx: reactTsIconUrl,
  yaml: yamlIconUrl,
  yml: yamlIconUrl,
  jsx: reactIconUrl,
}

const folderIconMap: Record<string, { closed: string; open: string }> = {
  '.git': { closed: folderGitIconUrl, open: folderGitOpenIconUrl },
  '.github': { closed: folderGitIconUrl, open: folderGitOpenIconUrl },
  assets: { closed: folderImagesIconUrl, open: folderImagesOpenIconUrl },
  components: { closed: folderComponentsIconUrl, open: folderComponentsOpenIconUrl },
  crates: { closed: folderPackagesIconUrl, open: folderPackagesOpenIconUrl },
  design: { closed: folderImagesIconUrl, open: folderImagesOpenIconUrl },
  docs: { closed: folderDocsIconUrl, open: folderDocsOpenIconUrl },
  images: { closed: folderImagesIconUrl, open: folderImagesOpenIconUrl },
  lib: { closed: folderLibIconUrl, open: folderLibOpenIconUrl },
  package: { closed: folderPackagesIconUrl, open: folderPackagesOpenIconUrl },
  packages: { closed: folderPackagesIconUrl, open: folderPackagesOpenIconUrl },
  public: { closed: folderPublicIconUrl, open: folderPublicOpenIconUrl },
  src: { closed: folderSrcIconUrl, open: folderSrcOpenIconUrl },
  test: { closed: folderTestIconUrl, open: folderTestOpenIconUrl },
  tests: { closed: folderTestIconUrl, open: folderTestOpenIconUrl },
}

function getFileExtensionMatches(name: string) {
  const normalized = name.toLowerCase()
  const parts = normalized.split('.').filter(Boolean)
  const matches: string[] = []

  for (let index = 0; index < parts.length; index += 1) {
    matches.push(parts.slice(index).join('.'))
  }

  return matches
}

function resolveFileIconUrl(name: string) {
  const normalized = name.toLowerCase()

  if (fileNameIconMap[normalized]) {
    return fileNameIconMap[normalized]
  }

  for (const extension of getFileExtensionMatches(normalized)) {
    if (fileExtensionIconMap[extension]) {
      return fileExtensionIconMap[extension]
    }
  }

  return fileIconUrl
}

function resolveFolderIconUrl(name: string, open: boolean) {
  const normalized = name.toLowerCase()
  const variant = folderIconMap[normalized]
  return variant ? (open ? variant.open : variant.closed) : open ? folderOpenIconUrl : folderIconUrl
}

export function RepoEntryIcon({
  kind,
  name,
  open = false,
  className = 'h-4 w-4',
}: {
  kind: 'tree' | 'blob'
  name: string
  open?: boolean
  className?: string
}) {
  const url = kind === 'tree' ? resolveFolderIconUrl(name, open) : resolveFileIconUrl(name)

  if (url) {
    return <img alt="" aria-hidden="true" className={`${className} shrink-0 object-contain`} src={url} />
  }

  if (kind === 'tree') {
    const FallbackIcon = open ? FolderOpen : Folder
    return <FallbackIcon aria-hidden="true" className={`${className} shrink-0 text-accent`} strokeWidth={1.75} />
  }

  return <FileText aria-hidden="true" className={`${className} shrink-0 text-fg-muted`} strokeWidth={1.75} />
}
