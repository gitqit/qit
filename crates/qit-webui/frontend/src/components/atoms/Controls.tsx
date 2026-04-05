import {
  forwardRef,
  type ButtonHTMLAttributes,
  type PropsWithChildren,
  type ReactNode,
} from 'react'

export type ButtonTone = 'primary' | 'muted' | 'danger'

type ButtonProps = PropsWithChildren<
  ButtonHTMLAttributes<HTMLButtonElement> & {
    tone?: ButtonTone
    icon?: ReactNode
    iconPosition?: 'leading' | 'trailing'
  }
>

type IconButtonProps = Omit<ButtonProps, 'children' | 'aria-label'> & {
  label: string
  icon: ReactNode
}

const buttonBase =
  'inline-flex items-center justify-center gap-2 rounded-token px-3.5 py-2 text-sm font-semibold transition-colors outline-none focus-visible:ring-2 focus-visible:ring-accent/30 focus-visible:ring-offset-2 focus-visible:ring-offset-canvas disabled:cursor-not-allowed disabled:opacity-50'

const buttonTones: Record<ButtonTone, string> = {
  primary:
    'border border-accent/70 bg-accent text-canvas shadow-[0_1px_0_rgba(255,255,255,0.06)_inset] hover:border-accent-strong hover:bg-accent-strong',
  muted:
    'border border-border bg-panel text-fg-muted hover:border-border-strong hover:bg-panel-subtle hover:text-fg',
  danger: 'border border-danger/40 bg-danger/10 text-danger hover:border-danger/55 hover:bg-danger/14',
}

function classNames(...values: Array<string | false | null | undefined>) {
  return values.filter(Boolean).join(' ')
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  {
    children,
    className,
    icon,
    iconPosition = 'leading',
    tone = 'primary',
    type = 'button',
    ...props
  },
  ref,
) {
  return (
    <button
      {...props}
      className={classNames(buttonBase, buttonTones[tone], className)}
      ref={ref}
      type={type}
    >
      {icon && iconPosition === 'leading' ? (
        <span aria-hidden="true" className="shrink-0">
          {icon}
        </span>
      ) : null}
      {children ? <span>{children}</span> : null}
      {icon && iconPosition === 'trailing' ? (
        <span aria-hidden="true" className="shrink-0">
          {icon}
        </span>
      ) : null}
    </button>
  )
})

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { className, label, title, ...props },
  ref,
) {
  return (
    <Button
      {...props}
      aria-label={label}
      className={classNames('aspect-square px-2.5 py-2', className)}
      ref={ref}
      title={title ?? label}
    />
  )
})

export function Badge({
  children,
  tone = 'muted',
  icon,
}: PropsWithChildren<{ tone?: 'muted' | 'success' | 'danger' | 'accent'; icon?: ReactNode }>) {
  const styles = {
    muted: 'border-border bg-panel-subtle text-fg-muted',
    success: 'border-success/25 bg-success/10 text-success',
    danger: 'border-danger/25 bg-danger/10 text-danger',
    accent: 'border-accent/25 bg-accent/10 text-accent-strong',
  }

  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-semibold ${styles[tone]}`}
    >
      {icon ? <span aria-hidden="true" className="shrink-0">{icon}</span> : null}
      {children}
    </span>
  )
}

export function Panel({
  children,
  title,
  subtitle,
  action,
}: PropsWithChildren<{ title: string; subtitle?: string; action?: ReactNode }>) {
  return (
    <section className="overflow-hidden rounded-[var(--radius-lg)] border border-border/90 bg-panel shadow-[inset_0_1px_0_rgba(255,255,255,0.02)]">
      <header className="flex flex-col gap-3 border-b border-border/80 bg-canvas-raised/65 px-5 py-4 sm:flex-row sm:items-start sm:justify-between">
        <div className="space-y-1">
          <h2 className="text-lg font-semibold tracking-tight text-fg">{title}</h2>
          {subtitle ? <p className="max-w-3xl text-sm leading-6 text-fg-muted">{subtitle}</p> : null}
        </div>
        {action}
      </header>
      <div className="px-5 py-5">{children}</div>
    </section>
  )
}

export function EmptyState({
  title,
  message,
}: {
  title: string
  message: string
}) {
  return (
    <div className="rounded-[var(--radius-lg)] border border-dashed border-border/90 bg-panel-subtle/75 px-6 py-10 text-center">
      <p className="text-base font-semibold text-fg">{title}</p>
      <p className="mx-auto mt-2 max-w-xl text-sm leading-6 text-fg-muted">{message}</p>
    </div>
  )
}

export function Spinner() {
  return (
    <span className="inline-flex h-5 w-5 animate-spin rounded-full border-2 border-accent/35 border-t-accent" />
  )
}
