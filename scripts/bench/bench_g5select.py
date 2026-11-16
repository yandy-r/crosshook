"""G5 photon-frame selection: which presented toplevel frame answers one injected input.

The app marks the render with a clock-synced time (`render_ns`) that is only known to within
`uncertainty_ns`, so the marks only decide WHICH frame is chosen, never how long it took.
`latency_ms` is always `(presented_ns - inject_ns) / 1e6`. A toplevel commit inside the render
window cannot be told apart from the change frame, so that trial is `ambiguous` and not measured.
A render mark that predates the injected key (beyond the uncertainty) is `early` (set by the
driver) and a trial with no presented frame is a `timeout`: neither is measured. All three count
toward the partial rule over every trial, so dropping slow or odd trials cannot bias a row.
"""
from __future__ import annotations

from dataclasses import dataclass

from bench_capture import CaptureError, stats
from bench_present import Commit, PresentLog, Presented

MEASURED = "measured"
AMBIGUOUS = "ambiguous"
TIMEOUT = "timeout"
EARLY = "early"
MIN_MEASURED = 10
AMBIGUOUS_LIMIT = 0.10


@dataclass(frozen=True)
class Trial:
    status: str  # measured | ambiguous | timeout | early
    commit_ns: int | None = None
    presented_ns: int | None = None
    latency_ms: float | None = None  # set only for a measured trial


def _candidates(logs: list[PresentLog], inject_ns: int) -> list[tuple[Commit, PresentLog]]:
    """Toplevel commits with a buffer made at or after the inject, oldest first."""
    found = [(commit, log) for log in logs for commit in log.commits.values()
             if commit.toplevel and commit.has_buffer and commit.commit_ns >= inject_ns]
    return sorted(found, key=lambda pair: pair[0].commit_ns)


def _presented(commit: Commit, log: PresentLog, deadline_ns: int) -> Presented | None:
    """The commit's Presented record, unless it was discarded, never shown or shown after the deadline."""
    if commit.seq in log.discarded:
        return None
    record = log.presented.get(commit.seq)
    return record if record is not None and record.presented_ns <= deadline_ns else None


def _first_presented(candidates: list[tuple[Commit, PresentLog]], not_before_ns: int,
                     deadline_ns: int) -> tuple[Commit, Presented] | None:
    for commit, log in candidates:
        if commit.commit_ns >= not_before_ns:
            record = _presented(commit, log, deadline_ns)
            if record is not None:
                return commit, record
    return None


def select_trial(logs: list[PresentLog], inject_ns: int, render_ns: int, uncertainty_ns: int,
                 deadline_ns: int) -> Trial:
    low, high = render_ns - uncertainty_ns, render_ns + uncertainty_ns
    candidates = _candidates(logs, inject_ns)
    if any(low <= commit.commit_ns < high for commit, _ in candidates):
        # Diagnostics only: the first frame shown from the window on. No latency is claimed.
        found = _first_presented(candidates, low, deadline_ns)
        return Trial(AMBIGUOUS, *((found[0].commit_ns, found[1].presented_ns) if found else (None, None)))
    found = _first_presented(candidates, high, deadline_ns)
    if found is None:
        return Trial(TIMEOUT)
    commit, record = found
    return Trial(MEASURED, commit.commit_ns, record.presented_ns, (record.presented_ns - inject_ns) / 1e6)


def summarize(scenario: str, trials: list[Trial]) -> dict:
    measured = [trial.latency_ms for trial in trials if trial.status == MEASURED]
    counts = {status: sum(1 for trial in trials if trial.status == status) for status in (AMBIGUOUS, TIMEOUT, EARLY)}
    if len(measured) < MIN_MEASURED:
        raise CaptureError(f"CAPTURE_TOO_SHORT: {scenario}: {len(measured)} measured trials, need {MIN_MEASURED}")
    summary = stats(measured)
    partial = sum(counts.values()) > AMBIGUOUS_LIMIT * len(trials)
    return {"scenario": scenario, "p50_ms": summary["p50_ms"], "p95_ms": summary["p95_ms"],
            "p99_ms": summary["p99_ms"], "measured": len(measured), "ambiguous": counts[AMBIGUOUS],
            "timeout": counts[TIMEOUT], "early": counts[EARLY],
            "status": "partial" if partial else "captured"}
