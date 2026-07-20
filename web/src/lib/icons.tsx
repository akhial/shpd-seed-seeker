import type { ReactNode, SVGProps } from 'react'

// Icons vendored from Lucide (https://lucide.dev), ISC License. Only the few
// glyphs the UI needs are copied here so we don't pull in the whole package.

type IconProps = SVGProps<SVGSVGElement> & { size?: number }

function Icon({ size = 16, children, ...props }: IconProps & { children: ReactNode }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...props}
    >
      {children}
    </svg>
  )
}

/** Lucide `command` — the macOS ⌘ key. */
export function CommandIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3" />
    </Icon>
  )
}

/** Lucide `corner-down-left` — the Return / Enter key. */
export function ReturnIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M20 4v7a4 4 0 0 1-4 4H4" />
      <path d="m9 10-5 5 5 5" />
    </Icon>
  )
}

/** Lucide `git-fork` — a branching fork, marks branching-choice reward notes. */
export function ForkIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="18" r="3" />
      <circle cx="6" cy="6" r="3" />
      <circle cx="18" cy="6" r="3" />
      <path d="M18 9v2c0 .6-.4 1-1 1H7c-.6 0-1-.4-1-1V9" />
      <path d="M12 12v3" />
    </Icon>
  )
}
