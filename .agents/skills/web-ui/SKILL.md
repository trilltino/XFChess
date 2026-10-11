---
name: web-ui
description: Implement XFChess React/TypeScript UI with React Router, Motion, ECharts and icon libraries. Use for browser routes, animation, charts, accessible interactions and frontend integration contracts.
metadata:
  provenance: docs-derived
---

# Web UI integration

Read the target package.json, tsconfig and existing components. The website,
wallet, admin and visualization are separate Vite apps with different TypeScript
and dependency versions. Use their local npm scripts and lockfiles.

- Keep API and signing adapters typed and outside presentation components. Parse
  untrusted payloads at the boundary; TypeScript annotations do not validate JSON.
- Apply the installed React performance guide to client components. These apps
  are not Next.js applications: do not introduce server actions or Next imports.
- Preserve React Router's existing mode and major version. Model routes and loading,
  empty, denied and failure states explicitly; retain native link/history behavior.
- Effects own their listeners, subscriptions and abort controllers. Cancel on
  unmount and correlate asynchronous results with the action/account that started them.
- Use the installed framer-motion import surface, respecting reduced motion and
  focus. Do not switch to a new package merely because online examples use it.
- Initialize ECharts once per mounted container, resize with its container and
  dispose on unmount. Keep metric units and labels explicit; avoid passing secrets
  or unbounded raw event histories into chart options.
- Use existing Lucide/simple-icons assets with labels where needed. Test keyboard,
  focus, small windows and overlay interactions on the actual target app.

Use the installed Vite and Vitest skills for build/test mechanics. The website also
has Playwright tests; verify visible user outcomes and relevant API/console errors.

Sources: [TypeScript](https://www.typescriptlang.org/docs/handbook/intro.html),
[React](https://react.dev/learn),
[React Router 7](https://reactrouter.com/7.18.4/start/declarative/routing),
[Motion](https://motion.dev/docs/react),
[ECharts handbook](https://echarts.apache.org/handbook/en/),
[Lucide React](https://lucide.dev/guide/packages/lucide-react),
[Playwright](https://playwright.dev/docs/intro).
