import { useId, type ChangeEvent, type KeyboardEventHandler, type ReactNode } from 'react'

function classNames(...values: Array<string | false | null | undefined>) {
  return values.filter(Boolean).join(' ')
}

export function SectionHeader({
  eyebrow,
  title,
  detail,
}: {
  eyebrow?: string
  title: string
  detail?: string
}) {
  return (
    <div className="space-y-2">
      {eyebrow ? (
        <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">{eyebrow}</p>
      ) : null}
      <h1 className="text-3xl font-semibold tracking-tight text-fg sm:text-4xl">{title}</h1>
      {detail ? <p className="max-w-2xl text-base leading-7 text-fg-muted">{detail}</p> : null}
    </div>
  )
}

export function TextInput({
  id,
  label,
  value,
  onChange,
  placeholder,
  description,
  error,
  required = false,
  disabled = false,
  autoFocus = false,
  name,
  type = 'text',
  autoComplete,
  onBlur,
  onKeyDown,
}: {
  id?: string
  label: string
  value: string
  onChange: (value: string) => void
  placeholder?: string
  description?: string
  error?: string | null
  required?: boolean
  disabled?: boolean
  autoFocus?: boolean
  name?: string
  type?: string
  autoComplete?: string
  onBlur?: () => void
  onKeyDown?: KeyboardEventHandler<HTMLInputElement>
}) {
  const fallbackId = useId()
  const inputId = id ?? fallbackId
  const descriptionId = description ? `${inputId}-description` : undefined
  const errorId = error ? `${inputId}-error` : undefined
  const describedBy = [descriptionId, errorId].filter(Boolean).join(' ') || undefined

  return (
    <div className="space-y-2">
      <label className="block" htmlFor={inputId}>
        <span className="text-sm font-medium text-fg">{label}</span>
      </label>
      <input
        aria-describedby={describedBy}
        aria-invalid={error ? true : undefined}
        aria-required={required || undefined}
        autoComplete={autoComplete}
        autoFocus={autoFocus}
        className={classNames(
          'w-full rounded-token border bg-panel-subtle px-3.5 py-2.5 text-sm text-fg outline-none transition-colors placeholder:text-fg-subtle focus:border-accent focus:ring-2 focus:ring-accent/20',
          error ? 'border-danger/70 focus:border-danger focus:ring-danger/20' : 'border-border',
        )}
        disabled={disabled}
        id={inputId}
        name={name}
        onChange={(event: ChangeEvent<HTMLInputElement>) => onChange(event.target.value)}
        onBlur={onBlur}
        onKeyDown={onKeyDown}
        placeholder={placeholder}
        required={required}
        type={type}
        value={value}
      />
      {description ? (
        <p className="text-xs leading-5 text-fg-subtle" id={descriptionId}>
          {description}
        </p>
      ) : null}
      <FieldError id={errorId} message={error} />
    </div>
  )
}

export function TextArea({
  id,
  label,
  value,
  onChange,
  rows = 4,
  description,
  error,
  required = false,
  disabled = false,
  name,
  onBlur,
  onKeyDown,
}: {
  id?: string
  label: string
  value: string
  onChange: (value: string) => void
  rows?: number
  description?: string
  error?: string | null
  required?: boolean
  disabled?: boolean
  name?: string
  onBlur?: () => void
  onKeyDown?: KeyboardEventHandler<HTMLTextAreaElement>
}) {
  const fallbackId = useId()
  const textareaId = id ?? fallbackId
  const descriptionId = description ? `${textareaId}-description` : undefined
  const errorId = error ? `${textareaId}-error` : undefined
  const describedBy = [descriptionId, errorId].filter(Boolean).join(' ') || undefined

  return (
    <div className="space-y-2">
      <label className="block" htmlFor={textareaId}>
        <span className="text-sm font-medium text-fg">{label}</span>
      </label>
      <textarea
        aria-describedby={describedBy}
        aria-invalid={error ? true : undefined}
        aria-required={required || undefined}
        className={classNames(
          'w-full rounded-token border bg-panel-subtle px-3.5 py-2.5 text-sm text-fg outline-none transition-colors placeholder:text-fg-subtle focus:border-accent focus:ring-2 focus:ring-accent/20',
          error ? 'border-danger/70 focus:border-danger focus:ring-danger/20' : 'border-border',
        )}
        disabled={disabled}
        id={textareaId}
        name={name}
        onChange={(event: ChangeEvent<HTMLTextAreaElement>) => onChange(event.target.value)}
        onBlur={onBlur}
        onKeyDown={onKeyDown}
        required={required}
        rows={rows}
        value={value}
      />
      {description ? (
        <p className="text-xs leading-5 text-fg-subtle" id={descriptionId}>
          {description}
        </p>
      ) : null}
      <FieldError id={errorId} message={error} />
    </div>
  )
}

export function FieldError({
  id,
  message,
}: {
  id?: string
  message?: string | null
}) {
  if (!message) {
    return null
  }

  return (
    <p aria-live="polite" className="text-sm text-danger" id={id} role="alert">
      {message}
    </p>
  )
}

export function FormActions({ children, hint }: { children: ReactNode; hint?: ReactNode }) {
  return (
    <div className="flex flex-col gap-3 border-t border-border/80 pt-5 sm:flex-row sm:items-center sm:justify-between">
      <div className="text-xs leading-5 text-fg-subtle">{hint}</div>
      <div className="flex flex-col-reverse gap-3 sm:flex-row sm:justify-end">{children}</div>
    </div>
  )
}

export function KeyValueRow({
  label,
  value,
  icon,
}: {
  label: string
  value: string
  icon?: ReactNode
}) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-border/70 py-3.5 last:border-b-0">
      <span className="inline-flex items-center gap-2 text-sm text-fg-muted">
        {icon ? <span aria-hidden="true" className="shrink-0">{icon}</span> : null}
        <span>{label}</span>
      </span>
      <span className="truncate text-sm font-medium text-fg">{value}</span>
    </div>
  )
}
