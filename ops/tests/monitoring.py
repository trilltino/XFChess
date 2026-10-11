"""Verify the disposable monitoring Compose stack after its health checks pass."""
import base64
import json
import os
import time
from urllib.request import Request, urlopen

base = "http://127.0.0.1:13000"
auth = base64.b64encode(f"admin:{os.environ['GRAFANA_ADMIN_PASSWORD']}".encode()).decode()


def request(path, body=None):
    headers = {"Authorization": f"Basic {auth}", "Content-Type": "application/json"}
    data = json.dumps(body).encode() if body else None
    with urlopen(Request(base + path, data=data, headers=headers), timeout=5) as response:
        return json.load(response)


for attempt in range(30):
    try:
        assert request("/api/datasources/uid/prometheus/health")["status"] == "OK"
        break
    except Exception:
        if attempt == 29:
            raise
        time.sleep(1)

dashboard = request("/api/dashboards/uid/xfchess")
assert dashboard["dashboard"]["uid"] == "xfchess"
source = request("/api/datasources/uid/prometheus")
assert source["url"] == "http://prometheus:9090"
query = request("/api/ds/query", {
    "from": "now-5m", "to": "now", "queries": [{"refId": "A",
    "datasource": {"uid": "prometheus", "type": "prometheus"},
    "expr": "vector(1)", "instant": True, "range": False, "intervalMs": 1000, "maxDataPoints": 100}],
})
assert query["results"]["A"]["frames"][0]["data"]["values"][1][0] == 1
print("Grafana dashboard provisioning and Prometheus query passed.")
