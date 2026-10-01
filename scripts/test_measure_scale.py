"""Small samples must retain the slowest observations in reported tail latency."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("measure_scale", Path(__file__).with_name("measure-scale.py"))
scale = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scale)


class ScaleMetricsTests(unittest.TestCase):
    def test_nearest_rank_percentiles_include_small_sample_tails(self):
        samples = [500, 20, 40, 10, 30]
        self.assertEqual(scale.percentile(samples, .5), 30)
        self.assertEqual(scale.percentile(samples, .95), 500)
        self.assertEqual(scale.percentile(samples, .99), 500)
        self.assertEqual(scale.percentile([7], .99), 7)
        self.assertIsNone(scale.percentile([], .99))


if __name__ == "__main__":
    unittest.main()
