#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');

console.log(' Starting XFChess Tournament Admin Panel...');

// Change to the tournament-admin directory
process.chdir(path.join(__dirname, '..'));

// Start the dev server
const devServer = spawn('npm', ['run', 'dev'], {
  stdio: 'inherit',
  shell: true
});

devServer.on('close', (code) => {
  console.log(`Dev server exited with code ${code}`);
});

devServer.on('error', (err) => {
  console.error('Failed to start dev server:', err);
  process.exit(1);
});

// Handle process termination
process.on('SIGINT', () => {
  console.log('\n Shutting down admin panel...');
  devServer.kill('SIGINT');
});

process.on('SIGTERM', () => {
  console.log('\n Shutting down admin panel...');
  devServer.kill('SIGTERM');
});

