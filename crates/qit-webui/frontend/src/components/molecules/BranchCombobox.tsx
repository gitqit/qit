import {
  Combobox,
  ComboboxButton,
  ComboboxInput,
  ComboboxOption,
  ComboboxOptions,
} from '@headlessui/react'
import { Check, ChevronDown, GitBranch, Globe } from 'lucide-react'
import { useId, useMemo, useState } from 'react'
import type { BranchInfo } from '../../lib/types'
import { FieldError } from './Fields'

function classNames(...values: Array<string | false | null | undefined>) {
  return values.filter(Boolean).join(' ')
}

function shortSha(value: string) {
  return value.slice(0, 7)
}

export function BranchCombobox({
  label,
  value,
  onChange,
  branches,
  description,
  error,
  required = false,
  disabled = false,
  autoFocus = false,
  onBlur,
}: {
  label: string
  value: string
  onChange: (value: string) => void
  branches: BranchInfo[]
  description?: string
  error?: string | null
  required?: boolean
  disabled?: boolean
  autoFocus?: boolean
  onBlur?: () => void
}) {
  const inputId = useId()
  const descriptionId = description ? `${inputId}-description` : undefined
  const errorId = error ? `${inputId}-error` : undefined
  const describedBy = [descriptionId, errorId].filter(Boolean).join(' ') || undefined
  const [query, setQuery] = useState('')

  const filteredBranches = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase()

    if (!normalizedQuery) {
      return branches
    }

    return branches.filter((branch) => {
      const haystack = `${branch.name} ${branch.summary} ${branch.commit}`.toLowerCase()
      return haystack.includes(normalizedQuery)
    })
  }, [branches, query])

  return (
    <div className="space-y-2">
      <label className="block" htmlFor={inputId}>
        <span className="text-sm font-medium text-fg">{label}</span>
      </label>
      <Combobox
        disabled={disabled}
        immediate
        onChange={(nextValue: string | null) => {
          onChange(nextValue ?? '')
          setQuery('')
        }}
        value={value}
      >
        <div className="relative">
          <ComboboxInput
            aria-describedby={describedBy}
            aria-invalid={error ? true : undefined}
            aria-required={required || undefined}
            autoFocus={autoFocus}
            className={classNames(
              'w-full rounded-token border bg-panel-subtle px-3.5 py-2.5 pr-10 text-sm text-fg outline-none transition-colors placeholder:text-fg-subtle focus:border-accent focus:ring-2 focus:ring-accent/20',
              error ? 'border-danger/70 focus:border-danger focus:ring-danger/20' : 'border-border',
            )}
            displayValue={(branchName: string) => branchName}
            id={inputId}
            onBlur={onBlur}
            onChange={(event) => {
              const nextValue = event.target.value
              setQuery(nextValue)
              onChange(nextValue)
            }}
            placeholder="Select a branch"
            required={required}
          />
          <ComboboxButton className="absolute inset-y-0 right-0 flex items-center px-3 text-fg-subtle transition hover:text-fg">
            <ChevronDown aria-hidden="true" className="h-4 w-4" strokeWidth={1.9} />
          </ComboboxButton>
        </div>
        <ComboboxOptions
          anchor="bottom start"
          className="z-40 mt-2 max-h-72 w-(--input-width) min-w-full overflow-auto rounded-[var(--radius-lg)] border border-border bg-panel p-2 shadow-[var(--shadow-raised)] empty:invisible"
        >
          {filteredBranches.length === 0 ? (
            <div className="rounded-token px-3 py-2 text-sm text-fg-muted">
              No branches match this value.
            </div>
          ) : (
            filteredBranches.map((branch) => (
              <ComboboxOption
                className="group cursor-default rounded-token px-3 py-2 data-focus:bg-panel-subtle"
                key={branch.name}
                value={branch.name}
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0 space-y-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-sm font-medium text-fg">{branch.name}</span>
                      {branch.is_current ? (
                        <span className="inline-flex items-center gap-1 rounded-full border border-accent/30 bg-accent/12 px-2 py-0.5 text-[11px] font-semibold text-accent">
                          <GitBranch className="h-3 w-3" strokeWidth={1.85} />
                          Current
                        </span>
                      ) : null}
                      {branch.is_served ? (
                        <span className="inline-flex items-center gap-1 rounded-full border border-success/30 bg-success/12 px-2 py-0.5 text-[11px] font-semibold text-success">
                          <Globe className="h-3 w-3" strokeWidth={1.85} />
                          Served
                        </span>
                      ) : null}
                    </div>
                    <p className="truncate text-xs text-fg-subtle">{branch.summary || 'No commit summary available.'}</p>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="shrink-0 font-mono text-xs text-fg-subtle">{shortSha(branch.commit)}</span>
                    <Check
                      aria-hidden="true"
                      className="h-4 w-4 text-accent opacity-0 transition group-data-selected:opacity-100"
                      strokeWidth={1.9}
                    />
                  </div>
                </div>
              </ComboboxOption>
            ))
          )}
        </ComboboxOptions>
      </Combobox>
      {description ? (
        <p className="text-xs leading-5 text-fg-subtle" id={descriptionId}>
          {description}
        </p>
      ) : null}
      <FieldError id={errorId} message={error} />
    </div>
  )
}
