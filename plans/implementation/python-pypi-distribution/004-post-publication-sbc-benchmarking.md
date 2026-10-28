# M004 — Post-Publication SBC Runtime Benchmarking and Resource Policy

Status: blocked (operational: M003 first PyPI publication must succeed)
Primary class: polish
Source: `plans/subsystems/python-pypi-distribution-roadmap.md` M004.
Baseline: `main@13fcac0b7ac0125d38e4e9775ebe1d62028fda72` used for planning; benchmarking must pin the **published** `eggsearch` Python version and its installed wheel hashes.
Authority: ADR-0005, `architecture/python-library.md`, historical Python M002/M005 lifecycle guarantees and M003 release closure.
Primary rule: **publish first; measure on SBCs later; never compile Eggsearch or its wheel on the device.**

## Objective and evidence context

Characterize performance/footprint of the actual PyPI-installed native package on representative small ARM boards, validate SBC runtime behavior beyond CI QEMU smoke, then decide whether low-footprint policies should change. Physical-SBC benchmarking is explicitly **not a prerequisite** for M001–M003 or the first PyPI upload. The pre-release ARMv7 wheel remains correctly tagged and emulation-tested but publicly labeled *experimental* until hardware evidence exists.

The current binding initializes one process-scoped four-worker Tokio runtime with up to eight blocking threads and caps process-wide in-flight operations at 32. These are not necessarily optimal for low-memory/low-core hardware; they are characterization hypotheses, not grounds to alter production code before gathering measurements.

## Non-goals and invariants

No device-local Rust build, `maturin develop`, Cargo source compilation, compiler installation or sdist fallback. No mandatory on-device wheel production or large benchmark dependency stack. No changing results/ranking, security policy, ten-tool public API, trust semantics, cancellation/close or default platform support to chase an incidental synthetic benchmark. No portable performance threshold invented without baseline data. Do not equate one Raspberry Pi or Le Potato result with every SBC.

## Ordered work packages

1. Select at least one native aarch64 Linux SBC and one ARMv7 32-bit Linux environment as hardware access allows (e.g., Pi 5-class aarch64, Le Potato/Raspberry Pi OS ARMv7 hard-float), recording `uname -m`, `/etc/os-release`, glibc or musl version, Python version, physical RAM, core count, storage, swap and thermal/power mode. Devices are consumer/benchmark hosts only. If ARMv7 hardware is temporarily unavailable, classify that evidence as **blocked post-release**, without retroactively blocking M003.
2. Install **only the exact published wheel**, `python -m pip install --only-binary=:all: eggsearch==<published_version>`, confirm download URL/digest and `importlib.metadata`; instrument package import and each benchmark from a fresh process. Refuse a source distribution, ensure no Cargo/compiler invocation. Use a lightweight Python standard-library measurement harness (e.g., `time.perf_counter_ns`, `resource.getrusage`, `tracemalloc` for Python allocations, Linux `/proc/self/status`, `/proc/<pid>/smaps_rollup` for native RSS/PSS); optional `psutil` is not required.
3. Measure cold import and first Tokio initialization separately, initialization with default vs explicit Mode-Off config, idle RSS/PSS, memory after sequential search/fetch fixtures, 1/2/4/8/16/32 concurrent bounded calls, cancellation/drain latency, per-client close, sequential `asyncio.run` loops, and heavy-output JSON conversion near defined budget caps. Prefer deterministic local/recorded fixtures; external live-provider performance is a separate optional observation with provider failure context.
4. Capture CPU time, wall-time distributions (median, p95 and spread across repeated runs), peak RSS/PSS, thread counts, process FD count, total memory headroom, disk wheel footprint and observable thermal throttling. Vary default process-concurrency realistically, keep tests short and bounded, and use the same scenario configuration when comparing ARMv7/aarch64. Report exact command and metadata, not just averaged headline numbers.
5. Analyze whether fixed four Tokio workers/eight blocking threads or 32-admission budget is unnecessarily expensive on 1–2-core/low-memory hardware. If evidence supports adaptation, propose the narrowest backwards-compatible config/runtime setting, explicit default behavior, bounded lower/upper values and tests proving continued operation on larger hosts. Treat tuning as a **separate corrective/optimization plan and patch release**, not an unreviewed change to the published baseline.
6. Run a basic physical-device installed-wheel compatibility smoke beyond the benchmark: ten tool names, representative local Mode-Off evidence, no remote calls, malformed input rejection, error taxonomy, cancellation, close and repeated loops. Collect any exceptions and SDK behavior divergence; release a correction only after exact new candidate qualification. Update SBC support notes based on verified board/OS/interpreter pairs, not on product-family inference.

## Verification, stop conditions and closure

Run benchmark script with published wheel outside checkout, with an enforced `--only-binary=:all:` install and compilation-absence guard; check source SHA/version and wheel digest against M003 closure. Fail the device benchmark, not the published release, on out-of-memory, architecture mismatch or missing native platform; record failures. Keep profiling fixtures network-free unless clearly marked diagnostic. Preserve test logs, versioned scenario JSON/CSV summaries, resource statistics and notebook-free reproducible scripts under the designated testing/benchmarks documentation rather than generated tracked debris.

Closure `plans/closure/python-pypi-distribution/004-status.md` requires hardware/platform inventory, published wheel identity, commands, distributional data including RSS/PSS, first-import/first-operation latency and cancellation/thread metrics, correctness smoke, limitations, experimental-status recommendation and follow-up plans for any measured issues. Update roadmap and registry together. M004 may close without optimization if measurements show the current policy is adequate; evidence, not benchmark-driven refactoring, is the deliverable.
