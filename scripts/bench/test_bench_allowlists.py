"""The scenario and phase allowlists live in Rust, Python and TypeScript; they must never drift apart."""
import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_cli
import bench_drivers
import bench_events

NATIVE = Path(__file__).resolve().parents[2] / "src/crosshook-native"
BENCH_RS = NATIVE / "crates/crosshook-app/src/bench"
BENCH_TS = NATIVE / "src/lib/bench"

ARM = re.compile(r'^\s*Self::\w+ => "([a-z_]+)",\s*$', re.MULTILINE)
CONSTANT = re.compile(r"^export const (\w+) = '([a-z_]+)';", re.MULTILINE)
MAP_KEY = re.compile(r"^\s*\[(?:'([a-z_]+)'|([A-Z_]+)),\s*\w+\],\s*$", re.MULTILINE)
UNION_MEMBER = re.compile(r"^\s*\| '([a-z_]+)'", re.MULTILINE)
PHASE_UNION = re.compile(r"export type BenchMarkPhase =(.*?);", re.DOTALL)
FIXTURE_ROWS = NATIVE / "crates/crosshook-core/examples/bench_fixtures/tests.rs"
FIXTURE_MIN_ROWS = re.compile(r"const HEALTH_MIN_ROWS: usize = (\d+);")


def read(path: Path) -> str:
    if not path.is_file():
        raise AssertionError(f"allowlist source not found: {path}")
    return path.read_text(encoding="utf-8")


def rust_names(path: Path) -> set[str]:
    """The tokens of the `fn name(self)` match arms (`Self::Variant => "token",`) in `path`."""
    text = read(path)
    start = text.find("fn name(self)")
    end = text.find("\n    }\n", start)
    if start < 0 or end < 0:
        raise AssertionError(f"{path.name}: no `fn name(self)` match found; update test_bench_allowlists.py")
    names = set(ARM.findall(text[start:end]))
    if not names:
        raise AssertionError(f"{path.name}: `fn name(self)` has no `Self::X => \"token\"` arms")
    return names


def ts_constants(*files: str) -> dict[str, str]:
    return {name: token for file in files for name, token in CONSTANT.findall(read(BENCH_TS / file))}


class RustAllowlistTests(unittest.TestCase):
    def test_scenarios_match_the_runner_reader(self):
        self.assertEqual(rust_names(BENCH_RS / "scenario.rs"), set(bench_events.SCENARIOS))

    def test_phases_match_the_runner_reader(self):
        self.assertEqual(rust_names(BENCH_RS / "mark.rs"), set(bench_events.PHASES))


class TypeScriptAllowlistTests(unittest.TestCase):
    def test_phases_match_the_runner_reader(self):
        declaration = PHASE_UNION.search(read(BENCH_TS / "benchMark.ts"))
        self.assertIsNotNone(declaration, "benchMark.ts: no `export type BenchMarkPhase = ...;` found; update this test")
        members = set(UNION_MEMBER.findall(declaration.group(1)))
        self.assertTrue(members, "benchMark.ts: BenchMarkPhase has no `| 'phase'` members; update this test")
        self.assertEqual(members, set(bench_events.PHASES))

    def test_scenarios_match_the_runner_reader(self):
        constants = ts_constants("healthTable.ts", "healthScrollProbe.ts", "replayProbe.ts")
        entries = MAP_KEY.findall(read(BENCH_TS / "benchScenarios.ts"))
        self.assertTrue(entries, "benchScenarios.ts: no SCENARIOS map entries found; update this test")
        g5 = {literal or constants[name] for literal, name in entries}
        self.assertEqual(g5, set(bench_events.G5_SCENARIOS))
        extras = {constants["HEALTH_SCROLL_SCENARIO"], constants["REPLAY_SCENARIO"]}
        self.assertEqual(g5 | extras, set(bench_events.SCENARIOS))

    def test_the_health_row_threshold_and_wait_mirror_the_runner(self):
        text = read(BENCH_TS / "healthTable.ts")
        rows = re.search(r"^export const HEALTH_MIN_ROWS = (\d+);", text, re.MULTILINE)
        wait = re.search(r"^export const HEALTH_WAIT_MS = ([\d_]+);", text, re.MULTILINE)
        self.assertIsNotNone(rows, "healthTable.ts: HEALTH_MIN_ROWS not found")
        self.assertIsNotNone(wait, "healthTable.ts: HEALTH_WAIT_MS not found")
        self.assertEqual(int(rows.group(1)), bench_events.HEALTH_MIN_ROWS)
        self.assertEqual(int(wait.group(1).replace("_", "")), bench_events.HEALTH_WAIT_S * 1000)
        fixture = FIXTURE_MIN_ROWS.search(read(FIXTURE_ROWS))
        self.assertIsNotNone(fixture, "bench_fixtures/tests.rs: HEALTH_MIN_ROWS not found; update this test")
        self.assertEqual(int(fixture.group(1)), bench_events.HEALTH_MIN_ROWS)


class RunnerAllowlistTests(unittest.TestCase):
    def test_every_cli_scenario_has_a_driver(self):
        self.assertEqual(set(bench_cli.SCENARIOS["G4"] + bench_cli.SUPPLEMENTAL_SCENARIOS["G4"]),
                         set(bench_drivers.SCENARIOS))

    def test_the_app_scenario_of_each_opt_in_scenario_is_known_to_the_reader(self):
        self.assertIn(bench_drivers.HEALTH_SCENARIO, bench_events.SCENARIOS)
        self.assertIn(bench_cli.SUPPLEMENTAL_SCENARIOS["G5"][0], bench_events.G5_SCENARIOS)


if __name__ == "__main__":
    unittest.main()
