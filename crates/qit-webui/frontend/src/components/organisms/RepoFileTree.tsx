import { useMemo } from 'react'
import { ChevronRight } from 'lucide-react'
import { Tree } from 'react-arborist'
import type { TreeEntry } from '../../lib/types'
import { RepoEntryIcon } from '../../lib/fileIcons'

type FileTreeNode = {
  id: string
  name: string
  path: string
  kind: 'tree' | 'blob'
  children?: FileTreeNode[]
}

function sortTreeEntries(entries: TreeEntry[]) {
  return [...entries].sort((left, right) => {
    if (left.kind !== right.kind) {
      return left.kind === 'tree' ? -1 : 1
    }
    return left.name.localeCompare(right.name, undefined, { sensitivity: 'base' })
  })
}

function buildTreeNodes(path: string, treeCache: Record<string, TreeEntry[]>): FileTreeNode[] {
  const entries = sortTreeEntries(treeCache[path] ?? [])
  return entries.map((entry) => ({
    id: entry.path,
    name: entry.name,
    path: entry.path,
    kind: entry.kind,
    children: entry.kind === 'tree' ? buildTreeNodes(entry.path, treeCache) : undefined,
  }))
}

export function RepoFileTree({
  activePath,
  currentPath,
  treeCache,
  onLoadPath,
  onOpenFile,
}: {
  activePath: string | null
  currentPath: string
  treeCache: Record<string, TreeEntry[]>
  onLoadPath: (path: string) => Promise<void>
  onOpenFile: (path: string) => Promise<void>
}) {
  const data = useMemo(() => buildTreeNodes('', treeCache), [treeCache])
  const selectedId = activePath ?? (currentPath || undefined)

  return (
    <div className="h-full min-h-[24rem] overflow-hidden rounded-[var(--radius-lg)] border border-border bg-panel">
      <Tree<FileTreeNode>
        data={data}
        disableDrag
        disableDrop
        height={640}
        indent={18}
        openByDefault={false}
        overscanCount={8}
        padding={8}
        rowHeight={30}
        selection={selectedId}
        width="100%"
      >
        {({ node, style }) => {
          const isDirectory = node.data.kind === 'tree'
          return (
            <div style={style}>
              <button
                className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm ${
                  node.isSelected ? 'bg-accent/12 text-fg' : 'text-fg-muted hover:bg-panel-subtle'
                }`}
                onClick={async () => {
                  if (isDirectory) {
                    await onLoadPath(node.data.path)
                    node.toggle()
                    return
                  }

                  await onOpenFile(node.data.path)
                }}
                type="button"
              >
                <span className="flex h-4 w-4 items-center justify-center">
                  {isDirectory ? (
                    <ChevronRight
                      aria-hidden="true"
                      className={`h-3.5 w-3.5 text-fg-subtle transition ${node.isOpen ? 'rotate-90' : ''}`}
                      strokeWidth={1.85}
                    />
                  ) : null}
                </span>
                <RepoEntryIcon
                  className="h-4 w-4"
                  kind={node.data.kind}
                  name={node.data.name}
                  open={isDirectory && node.isOpen}
                />
                <span className="truncate">{node.data.name}</span>
              </button>
            </div>
          )
        }}
      </Tree>
    </div>
  )
}
