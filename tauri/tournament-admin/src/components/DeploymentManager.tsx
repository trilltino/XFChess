import { useEffect, useState } from "react";
import { getOpsConfig } from "../config/environments";
import { useAuth } from "../hooks/useAuth";
import { apiClient } from "../services/api";
import { deploymentCommands } from "../services/operations";
import { startPolling } from "../services/poll";

export default function DeploymentManager() {
  const { authState } = useAuth();
  const ops = getOpsConfig();
  const commands = deploymentCommands(ops);
  const [health, setHealth] = useState<{ status: string; git_sha: string; version: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => startPolling(async active => {
    const result = await apiClient.getHealth();
    if (!active()) return;
    setHealth(result.ok ? result.data ?? null : null);
    setError(result.ok ? null : result.error?.message ?? "Backend unavailable");
  }, 15000), [authState.backend_url]);
  return <div style={{ padding: "1.5rem", display: "grid", gap: "1.5rem" }}>
    <h2>Deployment</h2>
    <p>Connected backend: <strong>{authState.env.toUpperCase()}</strong> · {authState.backend_url}</p>
    <p>{error ?? (health ? `Health: ${health.status} · Version: ${health.version} · Commit: ${health.git_sha}` : "Checking backend…")}</p>
    <section>
      <h3>Production release</h3>
      <p>Run from the repository terminal with the deployment identity authorized for root on {ops.vpsHost || "your configured VPS"}.
        Deploy builds the committed source, snapshots databases and switches the backend and website together. The panel’s deploy login supports operations, not release installation.</p>
      {commands.problem ? <p role="alert">{commands.problem}</p> : <>
        <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{commands.deploy}</pre>
        <p>Restore the previous backend, website and server configuration; database contents stay current:</p>
        <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{commands.rollback}</pre>
      </>}
    </section>
    <section>
      <h3>Monitoring</h3>
      <p>Production dashboards listen on the VPS loopback. Open a separate monitoring tunnel with the deploy login:</p>
      {commands.monitoring && <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{commands.monitoring}</pre>}
      <p>Then open Grafana at http://127.0.0.1:3000 or Prometheus at http://127.0.0.1:9090.
        The game admin tunnel forwards only the backend API.</p>
    </section>
  </div>;
}
