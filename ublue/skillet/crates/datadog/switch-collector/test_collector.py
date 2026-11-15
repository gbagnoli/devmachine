import unittest

from collector import Snapshot, poll, readings


class CollectorTests(unittest.TestCase):
    def test_allowlist_and_documented_units(self):
        result = readings(
            {
                "port_1_status": "on",
                "port_1_connection_speed": 125,
                "port_1_sum_rx_mbytes": 2,
                "port_2_status": "off",
                "port_1_password": "private",
                "port_3_sum_rx_mbytes": float("nan"),
            }
        )
        self.assertIn('netgear_port_link_up{port="1"} 1', result)
        self.assertIn(
            'netgear_port_connection_speed_megabits_per_second{port="1"} 125', result
        )
        self.assertIn('netgear_port_link_up{port="2"} 0', result)
        self.assertNotIn("private", str(result))
        self.assertNotIn("nan", str(result))

    def test_failure_clears_stale_data(self):
        snapshot = Snapshot()
        snapshot.update({"port_1_status": "on"}, 42)

        class Broken:
            def get_switch_infos(self):
                raise RuntimeError("secret")

        self.assertFalse(poll(snapshot, Broken()))
        output = snapshot.render(now=43).decode()
        self.assertIn("netgear_collection_success 0", output)
        self.assertIn("netgear_last_success_timestamp_seconds 42", output)
        self.assertNotIn("netgear_port_link_up", output)
        self.assertNotIn("secret", output)

    def test_stuck_worker_does_not_expose_stale_readings(self):
        snapshot = Snapshot()
        snapshot.update({"port_1_status": "on"}, 42)
        self.assertIn("netgear_collection_success 1", snapshot.render(now=43).decode())
        output = snapshot.render(now=343).decode()
        self.assertIn("netgear_collection_success 0", output)
        self.assertNotIn("netgear_port_link_up", output)

    def test_exposition_declares_types_for_agent(self):
        snapshot = Snapshot()
        snapshot.update({"port_1_status": "on", "port_2_status": "off"}, 42)
        output = snapshot.render(now=43).decode()
        self.assertIn("# TYPE netgear_collection_success gauge", output)
        self.assertIn("# TYPE netgear_last_success_timestamp_seconds gauge", output)
        self.assertEqual(output.count("# TYPE netgear_port_link_up gauge"), 1)

    def test_multiple_ports_remain_contiguous_within_each_metric_family(self):
        snapshot = Snapshot()
        snapshot.update(
            {
                f"port_{port}_{field}": value
                for port in range(1, 9)
                for field, value in (("status", "on"), ("connection_speed", 1000))
            },
            42,
        )
        family = None
        counts = {}
        for line in snapshot.render(now=43).decode().splitlines():
            if line.startswith("# TYPE "):
                family = line.split()[2]
            elif line and not line.startswith("#"):
                name = line.split("{", 1)[0].split()[0]
                self.assertEqual(name, family)
                counts[name] = counts.get(name, 0) + 1
        self.assertEqual(counts["netgear_port_link_up"], 8)
        self.assertEqual(counts["netgear_port_connection_speed_megabits_per_second"], 8)

    def test_empty_data_is_failure(self):
        with self.assertRaises(ValueError):
            readings({})


class PortLabelTests(unittest.TestCase):
    def test_labels_preserve_numeric_port_and_unlabelled_ports(self):
        result = readings(
            {"port_1_status": "on", "port_2_status": "off"}, {"1": "uplink"}
        )
        self.assertIn('netgear_port_link_up{port="1",port_label="uplink"} 1', result)
        self.assertIn('netgear_port_link_up{port="2"} 0', result)


class ExtendedReadingsTests(unittest.TestCase):
    def test_poe_rates_and_aggregates_have_units_and_labels(self):
        data = {
            "port_1_speed_rx_mbytes": 1.25,
            "port_1_speed_tx_mbytes": 2.5,
            "port_1_speed_io_mbytes": 3.75,
            "port_1_poe_output_power": 6.1,
            "port_1_poe_voltage": 53,
            "port_1_poe_current": 115,
            "port_1_poe_temperature": -2,
            "port_1_poe_class": 4,
            "port_1_poe_power_active": "on",
            "port_1_poe_power_delivered": "on",
            "port_1_poe_status": "delivering_power",
            "port_1_poe_fault": "No Error",
            "sum_port_speed_rx": 1.25,
            "sum_port_speed_tx": 2.5,
            "sum_port_speed_io": 3.75,
            "sum_port_traffic_rx": 10,
            "sum_port_traffic_tx": 20,
        }
        snapshot = Snapshot({"1": "access-point"})
        snapshot.update(data, 42)
        output = snapshot.render(now=43).decode()
        for metric, value in [
            ("received_megabytes_per_second", 1.25),
            ("transmitted_megabytes_per_second", 2.5),
            ("io_megabytes_per_second", 3.75),
            ("poe_power_watts", 6.1),
            ("poe_voltage_volts", 53),
            ("poe_current_milliamperes", 115),
            ("poe_temperature_celsius", -2),
            ("poe_class", 4),
            ("poe_enabled", 1),
            ("poe_delivering_power", 1),
            ("poe_fault", 0),
        ]:
            self.assertIn(
                f'netgear_port_{metric}{{port="1",port_label="access-point"}} {value}',
                output,
            )
        self.assertIn('state="delivering_power"} 1', output)
        self.assertIn('state="disabled"} 0', output)
        self.assertIn("# TYPE netgear_io_megabytes_per_second gauge", output)
        self.assertIn("netgear_io_megabytes_per_second 3.75", output)
        self.assertIn("netgear_received_interval_megabytes 10", output)

    def test_missing_invalid_and_unknown_poe_readings_are_not_fabricated(self):
        result = readings(
            {
                "port_1_status": "on",
                "port_1_poe_output_power": None,
                "port_1_poe_voltage": float("inf"),
                "port_1_poe_current": -1,
                "port_1_poe_class": 9,
                "port_1_poe_status": "private-unrecognized",
                "port_1_poe_fault": "",
                "sum_port_speed_rx": True,
            }
        )
        self.assertEqual(result, ['netgear_port_link_up{port="1"} 1'])

    def test_poe_fault_and_disabled_are_observations_not_missing_data(self):
        result = readings(
            {
                "port_2_poe_output_power": 0,
                "port_2_poe_power_active": "off",
                "port_2_poe_fault": "Overload",
            }
        )
        self.assertIn('netgear_port_poe_power_watts{port="2"} 0', result)
        self.assertIn('netgear_port_poe_enabled{port="2"} 0', result)
        self.assertIn('netgear_port_poe_fault{port="2"} 1', result)
        self.assertNotIn("Overload", str(result))


if __name__ == "__main__":
    unittest.main()
