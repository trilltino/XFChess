#!/usr/bin/env node

// The tournament admin panel is desktop-only. There is no web dev server to
// launch — the UI is built to dist/ and rendered inside the Tauri
// "tournament-admin" window, served from the loopback-only wallet bridge.
console.error(
  'The admin panel no longer runs as a web process.\n' +
    'Launch the desktop window instead:\n' +
    '  just admin\n' +
    '  scripts\\start-tournament-admin.bat\n' +
    'After UI changes, rebuild with: npm run build'
);
process.exit(1);
