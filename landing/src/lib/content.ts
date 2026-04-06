export const repoUrl = 'https://github.com/gitqit/qit'
export const releasesUrl = `${repoUrl}/releases`
export const docsUrl = '/docs/install'

export const heroContent = {
  eyebrow: 'Qit',
  title: 'Quick Git for normal folders.',
  description:
    'Install Qit, point it at any folder, and share it over authenticated Git Smart HTTP without dropping `.git` into the working tree.',
  supportingNote:
    'Use Homebrew on macOS or Linux, grab a release binary on other platforms, or build from source if you prefer.',
  primaryCta: {
    href: releasesUrl,
    label: 'Download binaries',
  },
  secondaryCta: {
    href: docsUrl,
    label: 'Read the quick start',
  },
  quickStartLabel: 'After download',
  quickStartTitle: 'Share a folder in one command.',
  quickStartDescription:
    'Run the binary against any folder, get an authenticated Git remote plus a local Web UI, and keep the host tree unchanged until you choose to apply changes.',
  quickStartCommand: 'qit ./my-app',
  highlights: ['Normal folders stay normal', 'Per-session auth', 'Push first, apply later'],
} as const

export const featureContent = [
  {
    eyebrow: 'Normal folders stay normal',
    title: 'Serve a project without converting the host tree into a Git repo.',
    description:
      'Qit snapshots your folder into a hidden sidecar repository, so the published Git view stays cleanly separated from the files you are editing.',
  },
  {
    eyebrow: 'Standard Git clients',
    title: 'Clone and push through plain Git Smart HTTP.',
    description:
      'Developers can use normal `git clone`, `git fetch`, and `git push` flows instead of learning a custom sync protocol or installing a second app.',
  },
  {
    eyebrow: 'Auth by default',
    title: 'Every shared session starts with fresh credentials.',
    description:
      'Qit prints a temporary username and password on startup, keeps Web UI access gated for exposed sessions, and keeps the auth model intentionally simple.',
  },
  {
    eyebrow: 'Sidecar-first workflow',
    title: 'Review incoming changes before they touch the host folder.',
    description:
      'Pushed commits land in the sidecar first. You can apply them manually or enable auto-apply when the host tree is clean and on the expected branch.',
  },
] as const

export const previewContent = {
  eyebrow: 'Product preview',
  title: 'A cloneable session, not a mystery sync box.',
  description:
    'Qit shows example startup output, the session address, and a browser view into the published snapshot so collaborators can understand what is live before they clone.',
  terminalTitle: 'Example startup output',
  terminalLines: [
    '$ qit ./my-app',
    'Serving',
    '  path: ./my-app',
    '  branch: main',
    '  transport: local',
    '',
    'Web UI',
    '  local: http://127.0.0.1:8080/my-app',
    '',
    'Git',
    '  repo: http://127.0.0.1:8080/my-app/',
    '  clone: git clone http://session-owner:session-pass-7k2m@127.0.0.1:8080/my-app/',
    '',
    'Session',
    '  username: session-owner',
    '  password: session-pass-7k2m',
  ],
  flowSteps: [
    'Snapshot the folder into a hidden sidecar repository.',
    'Serve that snapshot over Git Smart HTTP.',
    'Accept pushes into the sidecar first, then apply back when ready.',
  ],
  uiCards: [
    {
      label: 'Served branch',
      value: 'main',
    },
    {
      label: 'Checked out locally',
      value: 'feature/landing-page',
    },
    {
      label: 'Session mode',
      value: 'Authenticated',
    },
    {
      label: 'Password output',
      value: 'Shown by default',
    },
  ],
} as const

export const faqContent = [
  {
    question: 'Do I need to turn my folder into a Git repository first?',
    answer:
      'No. Qit is built for normal folders. It snapshots the folder into a hidden sidecar repository and serves that Git view without dropping `.git` into the host tree.',
  },
  {
    question: 'How do collaborators connect to a Qit session?',
    answer:
      'They use standard Git tooling against the served URL. For shared sessions, Qit prints the temporary username and password when the server starts so you can hand those off alongside the clone command.',
  },
  {
    question: 'Where do pushes land?',
    answer:
      'Pushes update the sidecar repository first. That keeps the host folder safe until you explicitly apply the changes or enable auto-apply for a clean working tree.',
  },
  {
    question: 'Can I expose Qit beyond localhost?',
    answer:
      'Yes. Qit supports local, LAN, ngrok, and Tailscale-based transport adapters so you can choose between same-machine use, local-network sharing, and broader access.',
  },
  {
    question: 'Is this replacing GitHub or a full multi-user forge?',
    answer:
      'No. Qit is a lightweight sharing and review layer for a live folder. It focuses on serving, cloning, reviewing, and applying changes, not on replacing a full hosted platform.',
  },
] as const

export const footerLinks = [
  {
    label: 'GitHub',
    href: repoUrl,
  },
  {
    label: 'Quick start',
    href: docsUrl,
  },
  {
    label: 'Release binaries',
    href: releasesUrl,
  },
  {
    label: 'License',
    href: `${repoUrl}/blob/main/LICENSE`,
  },
] as const

export type HeroContent = typeof heroContent
export type FeatureContent = typeof featureContent
export type PreviewContent = typeof previewContent
export type FaqContent = typeof faqContent
export type FooterLinks = typeof footerLinks
