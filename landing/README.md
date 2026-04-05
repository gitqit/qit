# Qit Landing

Static GitHub Pages landing page for Qit, built with Vite, React, TypeScript, Tailwind CSS 4, Headless UI, and a Brad Frost-inspired atomic component structure.

## Scripts

- `npm run dev` starts the local Vite server.
- `npm run lint` runs ESLint.
- `npm run build` builds the production bundle.
- `npm run preview` serves the built site locally.

## Deployment

GitHub Pages deployment is handled by `.github/workflows/landing-pages.yml`.

- The workflow installs dependencies from this directory.
- `VITE_BASE_PATH` is set during the Pages build so asset URLs work for project pages.
