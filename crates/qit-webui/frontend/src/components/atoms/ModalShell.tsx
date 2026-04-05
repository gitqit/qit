import {
  Description,
  Dialog,
  DialogBackdrop,
  DialogPanel,
  DialogTitle,
} from '@headlessui/react'
import type { ReactNode } from 'react'

function classNames(...values: Array<string | false | null | undefined>) {
  return values.filter(Boolean).join(' ')
}

const sizeClasses = {
  sm: 'max-w-lg',
  md: 'max-w-2xl',
  lg: 'max-w-3xl',
} as const

export function ModalShell({
  open,
  onClose,
  title,
  description,
  size = 'md',
  allowClose = true,
  children,
}: {
  open: boolean
  onClose: () => void
  title: string
  description?: string
  size?: keyof typeof sizeClasses
  allowClose?: boolean
  children: ReactNode
}) {
  return (
    <Dialog className="relative z-30" onClose={allowClose ? onClose : () => {}} open={open}>
      <DialogBackdrop
        transition
        className="fixed inset-0 bg-black/72 backdrop-blur-sm duration-150 ease-out data-closed:opacity-0"
      />
      <div className="fixed inset-0 flex items-center justify-center p-4 sm:p-6">
        <DialogPanel
          transition
          className={classNames(
            'w-full max-h-[90vh] overflow-y-auto rounded-[var(--radius-lg)] border border-border bg-panel p-5 shadow-[var(--shadow-raised)] duration-150 ease-out data-closed:translate-y-2 data-closed:scale-[0.98] data-closed:opacity-0 sm:p-6',
            sizeClasses[size],
          )}
        >
          <div className="space-y-2">
            <DialogTitle className="text-xl font-semibold tracking-tight text-fg">{title}</DialogTitle>
            {description ? <Description className="max-w-2xl text-sm leading-6 text-fg-muted">{description}</Description> : null}
          </div>
          <div className="mt-5">{children}</div>
        </DialogPanel>
      </div>
    </Dialog>
  )
}
