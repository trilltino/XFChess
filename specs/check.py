"""Run bounded TLC cases and validate expected counterexamples."""

import argparse
import hashlib
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parent
TOOL = ROOT / ".tools" / "tla2tools-1.7.4.jar"
URL = "https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar"
SHA256 = "936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
SUITES = ("causal", "braid", "settlement")


def verified_tool():
    if not TOOL.is_file() or hashlib.sha256(TOOL.read_bytes()).hexdigest() != SHA256:
        raise ValueError("TLC is missing or has a different checksum; run --setup")
    return TOOL


def setup():
    TOOL.parent.mkdir(exist_ok=True)
    with urllib.request.urlopen(URL, timeout=60) as response:
        data = response.read()
    if hashlib.sha256(data).hexdigest() != SHA256:
        raise ValueError("Downloaded TLC failed checksum verification")
    TOOL.write_bytes(data)
    print(f"Installed {TOOL}")


def outcome_matches(returncode, output, expected):
    if expected == "PASS":
        return returncode == 0 and "Model checking completed. No error has been found." in output
    violations = re.findall(r"Invariant (\w+) is violated\.", output)
    return returncode == 12 and violations == [expected]


def check(config, java, timeout, destination):
    expected_line = config.read_text(encoding="utf-8").splitlines()[0]
    match = re.fullmatch(r"\\\* EXPECT: (\w+)", expected_line)
    if not match:
        raise ValueError(f"Missing expected outcome: {config}")
    expected = match.group(1)
    module = config.parent / (config.name.split(".")[0] + ".tla")
    if not module.is_file():
        raise ValueError(f"Missing model: {module}")
    command = [java, "-Xmx1g", "-cp", str(verified_tool()), "tlc2.TLC",
               "-deadlock", "-workers", "1",
               "-config", str(config), str(module)]
    destination.mkdir()
    log = destination / "tlc.log"
    try:
        with tempfile.TemporaryDirectory(prefix="states-", dir=destination) as metadata:
            if not Path(metadata).resolve().is_relative_to(destination.resolve()):
                raise ValueError("TLC metadata must stay inside its run directory")
            with log.open("w", encoding="utf-8") as output:
                result = subprocess.run(command + ["-metadir", metadata], cwd=destination,
                                        stdout=output, stderr=subprocess.STDOUT, timeout=timeout)
        text = log.read_text(encoding="utf-8", errors="replace")
        passed = outcome_matches(result.returncode, text, expected)
        detail = f"TLC exit {result.returncode}"
    except subprocess.TimeoutExpired:
        passed = False
        detail = f"timeout after {timeout}s"
    print(f"{'PASS' if passed else 'FAIL'} {config.stem} (expected {expected})", flush=True)
    if not passed:
        print(f"  {detail}; see {log}", flush=True)
    return passed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--setup", action="store_true", help="install pinned TLC, then exit")
    parser.add_argument("--suite", choices=SUITES)
    parser.add_argument("--case", help="configuration stem, as shown by --list")
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--timeout", type=int, default=600, help="seconds per case (default 600)")
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    if args.setup:
        setup()
        return 0
    cases = sorted(config for suite in SUITES if not args.suite or args.suite == suite
                   for config in (ROOT / suite).glob("*.cfg")
                   if not args.case or config.stem == args.case)
    if not cases:
        parser.error("no matching cases")
    if args.list:
        for config in cases:
            print(f"{config.parent.name}/{config.stem}")
        return 0
    verified_tool()
    java = shutil.which("java")
    if not java:
        raise ValueError("Java 11+ is required")
    runs = ROOT / ".runs"
    runs.mkdir(exist_ok=True)
    destination = Path(tempfile.mkdtemp(prefix="tlc-", dir=runs))
    print(f"Logs: {destination}", flush=True)
    results = [check(config, java, args.timeout, destination / config.stem) for config in cases]
    print(f"{sum(results)}/{len(results)} cases matched their expected outcomes.")
    return 0 if all(results) else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print(f"Error: {error}", file=sys.stderr)
        sys.exit(1)
