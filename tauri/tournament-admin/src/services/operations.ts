import { opsConfigProblem, opsSsh, type OpsConfig } from "../config/environments";

function psQuote(value: string): string {
  return `'${value.replace(/'/g, "''")}'`;
}

export function deploymentCommands(config: OpsConfig) {
  const problem = opsConfigProblem(config);
  if (problem) return { problem, deploy: null, rollback: null, monitoring: null };
  const args = `-Server ${psQuote(config.vpsHost)} -IdentityFile ${psQuote(config.sshKeyPath)}`;
  return {
    problem: null,
    deploy: `powershell -File ops\\scripts\\deploy.ps1 ${args}`,
    rollback: `powershell -File ops\\scripts\\rollback.ps1 ${args}`,
    monitoring: `ssh -i ${psQuote(config.sshKeyPath)} -N -L 3000:127.0.0.1:3000 -L 9090:127.0.0.1:9090 deploy@${config.vpsHost}`,
  };
}

export function operatorCommand(config: OpsConfig, command: string): string[] {
  const problem = opsConfigProblem(config);
  if (problem) throw new Error(problem);
  const ops = opsSsh(config);
  const quoted = `'${command.replace(/'/g, `'"'"'`)}'`;
  return ["-i", ops.key, "-o", "BatchMode=yes", "-o", "ConnectTimeout=8",
    "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=2",
    `${ops.user}@${ops.host}`, `timeout 30s bash -lc ${quoted}`];
}

export interface HostResources {
  cpu_percent: number | null;
  memory_used_bytes: number;
  memory_total_bytes: number;
  observed_at: number;
  source: string;
}

export function resourcePercentages(resources?: HostResources | null) {
  const bounded = (n: number | null | undefined) =>
    typeof n === "number" && Number.isFinite(n) && n >= 0 && n <= 100 ? Math.round(n) : null;
  return {
    cpu: bounded(resources?.cpu_percent),
    ram: resources && resources.memory_total_bytes > 0
      ? bounded(resources.memory_used_bytes / resources.memory_total_bytes * 100) : null,
  };
}
