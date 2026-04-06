import { ArrowDownUp, ArrowUp, ArrowDown } from 'lucide-react'
import {
  Children,
  type ComponentPropsWithoutRef,
  isValidElement,
  type ReactNode,
  useMemo,
  useState,
} from 'react'
import { classNames } from '../../lib/classNames'

type RowElementProps = {
  children?: ReactNode
}

type SectionElementProps = {
  children?: ReactNode
}

type ParsedCell = {
  content: ReactNode
  sortText: string
}

type ParsedTable = {
  headers: ParsedCell[]
  rows: ParsedCell[][]
}

function readTextContent(node: ReactNode): string {
  if (typeof node === 'string' || typeof node === 'number') {
    return String(node)
  }

  if (Array.isArray(node)) {
    return node.map((child) => readTextContent(child)).join('')
  }

  if (isValidElement<{ children?: ReactNode }>(node)) {
    return readTextContent(node.props.children)
  }

  return ''
}

function parseCells(children: ReactNode) {
  return Children.toArray(children)
    .filter((child): child is React.ReactElement<RowElementProps> => isValidElement<RowElementProps>(child))
    .map((cell) => ({
      content: cell.props.children,
      sortText: readTextContent(cell.props.children).trim(),
    }))
}

function parseRows(children: ReactNode) {
  return Children.toArray(children)
    .filter((child): child is React.ReactElement<RowElementProps> => isValidElement<RowElementProps>(child))
    .map((row) => parseCells(row.props.children))
    .filter((row) => row.length > 0)
}

function parseTable(children: ReactNode): ParsedTable | null {
  const sections = Children.toArray(children)
    .filter((child): child is React.ReactElement<SectionElementProps> => isValidElement<SectionElementProps>(child))

  const thead = sections.find((section) => section.type === 'thead')
  const tbody = sections.find((section) => section.type === 'tbody')

  if (!thead || !tbody) {
    return null
  }

  const headerRows = parseRows(thead.props.children)
  const bodyRows = parseRows(tbody.props.children)
  const headers = headerRows[0] ?? []

  if (headers.length === 0 || bodyRows.length === 0) {
    return null
  }

  return { headers, rows: bodyRows }
}

function normalizeValue(value: string) {
  return value.replace(/,/g, '').trim()
}

function compareValues(left: string, right: string) {
  const normalizedLeft = normalizeValue(left)
  const normalizedRight = normalizeValue(right)
  const leftNumber = Number(normalizedLeft)
  const rightNumber = Number(normalizedRight)
  const bothNumeric = normalizedLeft.length > 0
    && normalizedRight.length > 0
    && Number.isFinite(leftNumber)
    && Number.isFinite(rightNumber)

  if (bothNumeric) {
    return leftNumber - rightNumber
  }

  return normalizedLeft.localeCompare(normalizedRight, undefined, {
    numeric: true,
    sensitivity: 'base',
  })
}

export function DocsTable({
  children,
  className,
  ...props
}: ComponentPropsWithoutRef<'table'>) {
  const parsedTable = useMemo(() => parseTable(children), [children])
  const [sortState, setSortState] = useState<{
    columnIndex: number
    direction: 'asc' | 'desc'
  } | null>(null)

  if (!parsedTable) {
    return (
      <div className="docs-table-shell">
        <table className={classNames('docs-table', className)} {...props}>
          {children}
        </table>
      </div>
    )
  }

  const sortedRows = [...parsedTable.rows].sort((leftRow, rightRow) => {
    if (!sortState) {
      return 0
    }

    const leftValue = leftRow[sortState.columnIndex]?.sortText ?? ''
    const rightValue = rightRow[sortState.columnIndex]?.sortText ?? ''
    const comparison = compareValues(leftValue, rightValue)

    return sortState.direction === 'asc' ? comparison : -comparison
  })

  function toggleSort(columnIndex: number) {
    setSortState((current) => {
      if (!current || current.columnIndex !== columnIndex) {
        return { columnIndex, direction: 'asc' }
      }

      return {
        columnIndex,
        direction: current.direction === 'asc' ? 'desc' : 'asc',
      }
    })
  }

  return (
    <div className="docs-table-shell">
      <table className={classNames('docs-table', className)} {...props}>
        <thead>
          <tr>
            {parsedTable.headers.map((header, columnIndex) => {
              const isActive = sortState?.columnIndex === columnIndex
              const direction = isActive ? sortState?.direction : null

              return (
                <th key={`header-${columnIndex}`} scope="col">
                  <button
                    className={classNames('docs-table__sort-button', isActive && 'docs-table__sort-button--active')}
                    onClick={() => toggleSort(columnIndex)}
                    type="button"
                  >
                    <span>{header.content}</span>
                    {direction === 'asc' ? (
                      <ArrowUp aria-hidden="true" className="h-3.5 w-3.5" strokeWidth={2.2} />
                    ) : direction === 'desc' ? (
                      <ArrowDown aria-hidden="true" className="h-3.5 w-3.5" strokeWidth={2.2} />
                    ) : (
                      <ArrowDownUp aria-hidden="true" className="h-3.5 w-3.5 opacity-70" strokeWidth={2.1} />
                    )}
                  </button>
                </th>
              )
            })}
          </tr>
        </thead>
        <tbody>
          {sortedRows.map((row, rowIndex) => (
            <tr key={`row-${rowIndex}`}>
              {row.map((cell, cellIndex) => (
                <td key={`cell-${rowIndex}-${cellIndex}`}>{cell.content}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
