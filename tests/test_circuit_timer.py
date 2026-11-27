"""Circuit scheduling checks without root, network access, or route changes."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "bridge", Path(__file__).resolve().parents[1] / "quickshell-plugin/scripts/omatorsurf-bar.py")
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)


class CircuitTimerTests(unittest.TestCase):
    def test_minimum_and_custom_values(self):
        for value in ("5", "10", "15", "30", "7", "525600"):
            self.assertEqual(bridge.timer_minutes(value), int(value))
        for value in ("0", "4", "-5", "5.5", "nan", "", "５", "525601"):
            with self.assertRaises(ValueError):
                bridge.timer_minutes(value)

    def tick(self, timer, protected=True, failure=None):
        observation = {"status": {"enabled": protected}}
        with patch.object(bridge.time, "monotonic", return_value=1000), \
             patch.object(bridge.time, "time", return_value=2000), \
             patch.object(bridge, "publish") as publish, \
             patch.object(bridge, "run", side_effect=failure) as run, \
             patch.object(bridge, "snapshot", return_value=observation) as snapshot:
            result = bridge.update_timer(42, timer, observation)
        return result, run, publish, snapshot

    def test_not_early_and_off(self):
        for timer in ({"minutes": 5, "due": 1001}, {"minutes": 0, "due": 0}):
            result, run, _, _ = self.tick(timer)
            run.assert_not_called()
            self.assertEqual(result["circuit_timer"]["minutes"], timer["minutes"])

    def test_due_rotates_once_then_waits(self):
        timer = {"minutes": 5, "due": 1000}
        result, run, publish, _ = self.tick(timer)
        run.assert_called_once_with([bridge.BINARY, "new-circuit"], 100)
        self.assertEqual(timer["due"], 1300)
        self.assertEqual(result["circuit_timer"]["next_at"], 2300)
        self.assertEqual(publish.call_args_list[0].args[2], "timer.json")
        _, run, _, _ = self.tick(timer)
        run.assert_not_called()

    def test_unprotected_pauses_and_resets_countdown(self):
        timer = {"minutes": 10, "due": 900}
        _, run, _, _ = self.tick(timer, protected=False)
        run.assert_not_called()
        self.assertEqual(timer["due"], 1600)

    def test_failure_waits_full_interval(self):
        timer = {"minutes": 5, "due": 900}
        _, run, _, snapshot = self.tick(timer, failure=RuntimeError("Tor unavailable"))
        run.assert_called_once()
        snapshot.assert_called_once_with("Tor unavailable")
        self.assertEqual(timer["due"], 1300)


if __name__ == "__main__":
    unittest.main()
