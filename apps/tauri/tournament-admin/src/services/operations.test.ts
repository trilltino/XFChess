import { describe, expect, it } from "vitest";
import { deploymentCommands, operatorCommand, resourcePercentages } from "./operations";

describe("operations", () => {
  it("keeps SSH options fixed and quotes remote shell input, even input resembling a local option", () => {
    const args = operatorCommand({ vpsHost: "192.0.2.10", sshKeyPath: "C:/keys/operator" }, "-oProxyCommand=calc; echo 'ok'");
    expect(args.slice(-2)[0]).toBe("deploy@192.0.2.10");
    expect(args[args.length - 1]).toBe(`timeout 30s bash -lc '-oProxyCommand=calc; echo '"'"'ok'"'"''`);
    expect(args).toContain("BatchMode=yes");
  });
  it("uses the configured host and identity without exposing monitoring on the public VPS", () => {
    const commands = deploymentCommands({ vpsHost: "192.0.2.10", sshKeyPath: "C:/keys/operator" });
    expect(commands.deploy).toContain("-Server '192.0.2.10'");
    expect(commands.deploy).toContain("-IdentityFile 'C:/keys/operator'");
    expect(commands.monitoring).toContain("-L 9090:127.0.0.1:9090 deploy@192.0.2.10");
  });
  it("rejects missing or out-of-range connection settings", () => {
    for (const vpsHost of ["", "999.1.1.1", "1.2.3.4;calc"]) {
      expect(deploymentCommands({ vpsHost, sshKeyPath: "C:/keys/operator" }).deploy).toBeNull();
    }
  });
  it("keeps missing, invalid and zero readings distinct", () => {
    expect(resourcePercentages()).toEqual({ cpu: null, ram: null });
    expect(resourcePercentages({ cpu_percent: 0, memory_used_bytes: 0, memory_total_bytes: 10, observed_at: 1, source: "backend_host" })).toEqual({ cpu: 0, ram: 0 });
    expect(resourcePercentages({ cpu_percent: NaN, memory_used_bytes: 20, memory_total_bytes: 10, observed_at: 1, source: "backend_host" })).toEqual({ cpu: null, ram: null });
  });
});
