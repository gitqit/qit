import Editor, { DiffEditor } from '@monaco-editor/react'

function languageForPath(path: string) {
  const filename = path.split('/').at(-1)?.toLowerCase() ?? ''
  const extension = filename.includes('.') ? filename.split('.').pop() ?? '' : ''

  if (filename === 'dockerfile') return 'dockerfile'
  if (filename === 'makefile') return 'plaintext'

  const languages: Record<string, string> = {
    c: 'c',
    cc: 'cpp',
    cpp: 'cpp',
    css: 'css',
    go: 'go',
    h: 'cpp',
    hpp: 'cpp',
    htm: 'html',
    html: 'html',
    java: 'java',
    js: 'javascript',
    json: 'json',
    jsx: 'javascript',
    md: 'markdown',
    mdown: 'markdown',
    mkd: 'markdown',
    mkdn: 'markdown',
    markdown: 'markdown',
    py: 'python',
    rs: 'rust',
    sh: 'shell',
    sql: 'sql',
    toml: 'ini',
    ts: 'typescript',
    tsx: 'typescript',
    txt: 'plaintext',
    xml: 'xml',
    yml: 'yaml',
    yaml: 'yaml',
  }

  return languages[extension] ?? 'plaintext'
}

const baseOptions = {
  automaticLayout: true,
  cursorBlinking: 'solid' as const,
  diffWordWrap: 'off' as const,
  fontLigatures: true,
  fontSize: 12.5,
  lineDecorationsWidth: 10,
  lineNumbers: 'on' as const,
  lineNumbersMinChars: 4,
  minimap: { enabled: false },
  padding: { top: 16, bottom: 16 },
  readOnly: true,
  renderLineHighlight: 'none' as const,
  scrollBeyondLastLine: false,
  wordWrap: 'off' as const,
}

export function MonacoCodeSurface({
  path,
  value,
  height = 720,
}: {
  path: string
  value: string
  height?: number | string
}) {
  return (
    <Editor
      height={height}
      language={languageForPath(path)}
      options={baseOptions}
      path={path}
      theme="vs-dark"
      value={value}
    />
  )
}

export function MonacoDiffSurface({
  path,
  original,
  modified,
  height = 720,
}: {
  path: string
  original: string
  modified: string
  height?: number | string
}) {
  return (
    <DiffEditor
      height={height}
      language={languageForPath(path)}
      modified={modified}
      options={{
        ...baseOptions,
        renderSideBySide: true,
      }}
      original={original}
      theme="vs-dark"
    />
  )
}
