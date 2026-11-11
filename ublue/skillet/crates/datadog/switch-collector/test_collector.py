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

    def test_empty_data_is_failure(self):
        with self.assertRaises(ValueError):
            readings({})


if __name__ == "__main__":
    unittest.main()
