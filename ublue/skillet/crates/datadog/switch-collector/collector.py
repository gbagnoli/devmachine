"""Read-only NETGEAR switch metrics. Credentials are read from a Podman secret."""

import json
import math
import re
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from urllib.parse import urlsplit

PORT_KEY = re.compile(r"^port_([1-9][0-9]?)_(.+)$")
NAMES = {
    "connection_speed": "netgear_port_connection_speed_megabits_per_second",
    "sum_rx_mbytes": "netgear_port_received_megabytes_total",
    "sum_tx_mbytes": "netgear_port_transmitted_megabytes_total",
    "speed_rx_mbytes": "netgear_port_received_megabytes_per_second",
    "speed_tx_mbytes": "netgear_port_transmitted_megabytes_per_second",
    "speed_io_mbytes": "netgear_port_io_megabytes_per_second",
    "poe_output_power": "netgear_port_poe_power_watts",
    "poe_voltage": "netgear_port_poe_voltage_volts",
    "poe_current": "netgear_port_poe_current_milliamperes",
    "poe_temperature": "netgear_port_poe_temperature_celsius",
    "poe_class": "netgear_port_poe_class",
}
BOOLEAN_NAMES = {
    "status": "netgear_port_link_up",
    "poe_power_active": "netgear_port_poe_enabled",
    "poe_power_delivered": "netgear_port_poe_delivering_power",
}
AGGREGATE_NAMES = {
    "sum_port_speed_rx": "netgear_received_megabytes_per_second",
    "sum_port_speed_tx": "netgear_transmitted_megabytes_per_second",
    "sum_port_speed_io": "netgear_io_megabytes_per_second",
    "sum_port_traffic_rx": "netgear_received_interval_megabytes",
    "sum_port_traffic_tx": "netgear_transmitted_interval_megabytes",
}
POE_STATES = ("delivering_power", "searching", "disabled")


def valid_number(value, allow_negative=False):
    return (
        not isinstance(value, bool)
        and isinstance(value, (int, float))
        and math.isfinite(value)
        and (allow_negative or value >= 0)
    )


def readings(data, port_labels=None):
    """Emit bounded, documented metrics; missing readings stay absent."""
    result = []
    for key, value in sorted(data.items()):
        if key in AGGREGATE_NAMES:
            if valid_number(value):
                result.append(f"{AGGREGATE_NAMES[key]} {value}")
            continue
        match = PORT_KEY.fullmatch(key)
        if not match:
            continue
        port, field = match.groups()
        labels = f'port="{port}"'
        label = (port_labels or {}).get(port)
        if label is not None:
            labels += f",port_label={json.dumps(label)}"
        if field in BOOLEAN_NAMES:
            if value not in ("on", "off"):
                continue
            name, value = BOOLEAN_NAMES[field], int(value == "on")
        elif field == "poe_status":
            if value not in POE_STATES:
                continue
            for state in POE_STATES:
                result.append(
                    f'netgear_port_poe_status{{{labels},state="{state}"}} '
                    f"{int(value == state)}"
                )
            continue
        elif field == "poe_fault":
            if not isinstance(value, str) or not value.strip():
                continue
            name, value = "netgear_port_poe_fault", int(value.strip() != "No Error")
        elif field in NAMES:
            if not valid_number(value, allow_negative=field == "poe_temperature"):
                continue
            if field == "poe_class" and (value != int(value) or value > 8):
                continue
            name = NAMES[field]
        else:
            continue
        result.append(f"{name}{{{labels}}} {value}")
    if not result:
        raise ValueError("switch returned no recognized port data")
    return result


def typed_readings(lines):
    """Declare each family explicitly for Agent OpenMetrics ingestion.

    Cumulative switch readings stay gauges: resets must not create negative rates.
    """
    families = {}
    for line in lines:
        name = line.split("{", 1)[0].split()[0]
        families.setdefault(name, []).append(line)
    result = []
    for name, samples in sorted(families.items()):
        result.append(f"# TYPE {name} gauge")
        result.extend(samples)
    return result


class Snapshot:
    def __init__(self, port_labels=None):
        self.port_labels = port_labels or {}
        self.lock = threading.Lock()
        self.ok = False
        self.last_success = 0.0
        self.lines = []

    def update(self, data, now):
        lines = readings(data, self.port_labels)
        with self.lock:
            self.ok, self.last_success, self.lines = True, now, lines

    def fail(self):
        with self.lock:
            self.ok, self.lines = False, []

    def render(self, now=None):
        now = time.time() if now is None else now
        with self.lock:
            fresh = self.ok and now - self.last_success <= 300
            return (
                "\n".join(
                    [
                        "# TYPE netgear_collection_success gauge",
                        f"netgear_collection_success {int(fresh)}",
                        "# TYPE netgear_last_success_timestamp_seconds gauge",
                        f"netgear_last_success_timestamp_seconds {self.last_success}",
                        *(typed_readings(self.lines) if fresh else []),
                        "# EOF",
                        "",
                    ]
                )
            ).encode()


def poll(snapshot, connector):
    try:
        snapshot.update(connector.get_switch_infos(), time.time())
        return True
    except Exception:  # The library may raise transport, parser or login exceptions.
        snapshot.fail()
        return False


def run():
    # Import here so the pure observation logic can be tested without installing
    # external packages on the workstation.
    from py_netgear_plus import NetgearSwitchConnector

    config = json.loads(Path("/run/secrets/switch_config").read_text())
    endpoint = urlsplit(config["endpoint"])
    if (
        endpoint.scheme != "http"
        or not endpoint.hostname
        or endpoint.port not in (None, 80)
    ):
        raise ValueError("unsupported switch endpoint")
    port = int(config["metrics_port"])
    if not 9500 <= port < 9532:
        raise ValueError("invalid metrics port")
    snapshot = Snapshot(config.get("port_labels", {}))

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path != "/metrics":
                self.send_error(404)
                return
            body = snapshot.render()
            self.send_response(200)
            self.send_header(
                "Content-Type",
                "application/openmetrics-text; version=1.0.0; charset=utf-8",
            )
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, _format, *args):
            pass

    def worker():
        connector = None
        delay = 30
        while True:
            try:
                if connector is None:
                    connector = NetgearSwitchConnector(
                        endpoint.hostname, config["password"]
                    )
                    connector.autodetect_model()
                    connector.get_login_cookie()
                if poll(snapshot, connector):
                    delay = 30
                else:
                    connector = None
                    delay = min(delay * 2, 300)
            except Exception:
                snapshot.fail()
                connector = None
                delay = min(delay * 2, 300)
            time.sleep(delay)

    threading.Thread(target=worker, daemon=True).start()
    HTTPServer(("127.0.0.1", port), Handler).serve_forever()


if __name__ == "__main__":
    try:
        run()
    except Exception as error:
        # Never log exception text: upstream errors can contain credentials.
        print(f"collector stopped: {type(error).__name__}", file=sys.stderr)
        sys.exit(1)
