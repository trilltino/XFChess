import { execSync } from 'child_process';
import path from 'path';
import os from 'os';
import fs from 'fs';

const keyPath = path.join(os.homedir(), '.ssh', 'xfchess_vps');
if (!fs.existsSync(keyPath)) {
  console.error('SSH key not found at:', keyPath);
  process.exit(1);
}

const server = 'root@178.104.55.19';
const remoteDir = '/opt/xfchess/web/';
const distDir = path.resolve('dist');

console.log(`Building frontend...`);
execSync('npm run build', { stdio: 'inherit' });

console.log(`Uploading ${distDir}/* to ${server}:${remoteDir}...`);
execSync(`scp -i "${keyPath}" -o StrictHostKeyChecking=accept-new -r "${distDir}/"* ${server}:${remoteDir}`, { stdio: 'inherit' });

console.log('Fixing permissions on remote...');
execSync(`ssh -i "${keyPath}" -o StrictHostKeyChecking=accept-new ${server} "chmod -R o+rX /opt/xfchess/web"`, { stdio: 'inherit' });

console.log('Reloading nginx...');
execSync(`ssh -i "${keyPath}" -o StrictHostKeyChecking=accept-new ${server} "systemctl reload nginx"`, { stdio: 'inherit' });

console.log('Successfully deployed to live website!');
