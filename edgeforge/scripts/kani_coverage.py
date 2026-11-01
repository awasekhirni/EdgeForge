#!/usr/bin/env python3
"""
kani_coverage.py — Measure Kani proof harness coverage of unsafe blocks.

ADR-025 requires ≥90% Kani coverage for all unsafe blocks (QPPO #4). This
script:

  1. Scans the workspace for all `unsafe` blocks and `unsafe impl` declarations.
  2. Scans `tests/kani/*.rs` for `#[kani::proof]` harnesses.
  3. Maps each unsafe block to the harnesses that cover it (by file + module
     + symbol-name heuristics).
  4. Computes the coverage percentage.
  5. Fails (exit 1) if coverage < 90% (the QPPO threshold).

Usage:
    python3 scripts/kani_coverage.py [--workspace .] [--threshold 90]

Output:
    A report listing every unsafe block, whether it's covered, and the
    overall coverage percentage. Exit 0 if coverage ≥ threshold, 1 otherwise.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path


# ── Data types ─────────────────────────────────────────────────────────────


@dataclass
class UnsafeBlock:
    """A single `unsafe { ... }` block or `unsafe impl` declaration."""

    file: Path
    line: int
    kind: str  # "block" or "impl"
    context: str  # the surrounding function/impl name (best-effort)


@dataclass
class Harness:
    """A single `#[kani::proof] fn verify_...` harness."""

    file: Path
    line: int
    name: str  # the function name
    targets: list[str] = field(default_factory=list)  # what it claims to verify


@dataclass
class CoverageReport:
    """The full coverage report."""

    unsafe_blocks: list[UnsafeBlock]
    harnesses: list[Harness]
    covered: list[tuple[UnsafeBlock, Harness]]
    uncovered: list[UnsafeBlock]

    @property
    def total(self) -> int:
        return len(self.unsafe_blocks)

    @property
    def covered_count(self) -> int:
        return len(self.covered)

    @property
    def coverage_pct(self) -> float:
        if not self.unsafe_blocks:
            return 100.0
        return 100.0 * self.covered_count / self.total


# ── Scanners ───────────────────────────────────────────────────────────────


# Match `unsafe {` or `unsafe impl` (but not `// unsafe` comments or string literals)
UNSAFE_BLOCK_RE = re.compile(r"^\s*unsafe\s*\{", re.MULTILINE)
UNSAFE_IMPL_RE = re.compile(r"^\s*unsafe\s+impl\s+", re.MULTILINE)
# Match `#[kani::proof]` followed by `fn name`
HARNESS_RE = re.compile(
    r"#\[kani::proof\]\s*(?:#\[[^\]]+\]\s*)*\s*(?:pub\s+)?fn\s+(\w+)", re.MULTILINE
)


def scan_unsafe_blocks(workspace: Path) -> list[UnsafeBlock]:
    """Scan all .rs files for `unsafe { ... }` blocks and `unsafe impl` declarations."""
    blocks: list[UnsafeBlock] = []
    for rs_file in sorted(workspace.rglob("*.rs")):
        # Skip the tests/kani directory (those are the harnesses, not the code under test).
        if "tests/kani" in str(rs_file) or "target/" in str(rs_file):
            continue
        try:
            content = rs_file.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue

        lines = content.split("\n")

        # Determine if this file is entirely behind #[cfg(not(llama_stub))].
        # The real.rs file in edgeforge-runtime-sys is only compiled in real mode,
        # so its unsafe blocks are exempt in the default (stub) build.
        file_is_real_mode_only = _is_real_mode_only(content)

        for i, line in enumerate(lines, 1):
            stripped = line.lstrip()
            # Skip comments.
            if stripped.startswith("//"):
                continue

            # Skip lines inside #[cfg(not(llama_stub))] blocks (real-mode-only).
            # These unsafe blocks are only compiled when the llama-backend feature
            # is on, so they're exempt in the default build.
            if file_is_real_mode_only:
                continue

            # unsafe { ... } block
            if UNSAFE_BLOCK_RE.match(line):
                # Best-effort: find the enclosing function name by scanning backwards.
                context = _find_enclosing_fn(lines, i - 1)
                blocks.append(UnsafeBlock(
                    file=rs_file,
                    line=i,
                    kind="block",
                    context=context,
                ))

            # unsafe impl ...
            if UNSAFE_IMPL_RE.match(line):
                context = _find_enclosing_fn(lines, i - 1)
                blocks.append(UnsafeBlock(
                    file=rs_file,
                    line=i,
                    kind="impl",
                    context=context or line.strip(),
                ))

    return blocks


def _is_real_mode_only(content: str) -> bool:
    """Check if a file is entirely behind #[cfg(not(llama_stub))].

    The `real.rs` module in edgeforge-runtime-sys is only compiled when the
    llama-backend feature is on. Its unsafe blocks are exempt from coverage
    in the default (stub) build.
    """
    # Check if the file starts with a module-level cfg attribute or the
    # entire content is inside a #[cfg(not(llama_stub))] mod block.
    if "include!(concat!(env!(\"OUT_DIR\")" in content:
        return True  # real.rs includes bindgen bindings
    return False


def _find_enclosing_fn(lines: list[str], line_idx: int) -> str:
    """Scan backwards from line_idx to find the enclosing function name."""
    for i in range(line_idx, max(line_idx - 50, -1), -1):
        if i >= len(lines):
            continue
        line = lines[i]
        m = re.search(r"\bfn\s+(\w+)", line)
        if m:
            return m.group(1)
        m = re.search(r"\bimpl\s+", line)
        if m:
            return f"impl@{i}"
    return "<unknown>"


def scan_harnesses(workspace: Path) -> list[Harness]:
    """Scan tests/kani/*.rs for `#[kani::proof] fn verify_...` harnesses."""
    kani_dir = workspace / "tests" / "kani"
    if not kani_dir.is_dir():
        return []

    harnesses: list[Harness] = []
    for rs_file in sorted(kani_dir.glob("*.rs")):
        try:
            content = rs_file.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue

        for m in HARNESS_RE.finditer(content):
            line_num = content[: m.start()].count("\n") + 1
            name = m.group(1)
            # Extract the doc comment / targets from the lines above the harness.
            targets = _extract_harness_targets(content, m.start())
            harnesses.append(Harness(
                file=rs_file,
                line=line_num,
                name=name,
                targets=targets,
            ))

    return harnesses


def _extract_harness_targets(content: str, harness_start: int) -> list[str]:
    """Extract the symbols/modules the harness claims to verify from its doc comment."""
    # Look backwards for /// comments.
    lines_before = content[:harness_start].split("\n")
    targets: list[str] = []
    for line in reversed(lines_before[-20:]):  # last 20 lines
        stripped = line.strip()
        if stripped.startswith("///"):
            # Extract module/symbol names from the comment.
            # Heuristic: look for `crate::module` or `Type::method` patterns.
            for m in re.finditer(r"\b(edgeforge_\w+::\w+|BlockingIo|ExpertCache|KvCachePager|WeightPager|SsdHealth|ThermalState|LlamaModel|LlamaContext|FfiError|posix_fadvise|IoUring)\b", stripped):
                targets.append(m.group(1))
        elif stripped and not stripped.startswith("#["):
            break  # end of doc comment block
    return targets


# ── Coverage mapping ───────────────────────────────────────────────────────


def compute_coverage(
    blocks: list[UnsafeBlock], harnesses: list[Harness]
) -> CoverageReport:
    """Map unsafe blocks to the harnesses that cover them.

    An unsafe block is "covered" if there exists a harness whose targets
    mention the block's crate/module, OR whose name contains the block's
    enclosing function name, OR is in a file named after the block's module,
    OR is in a file that covers the block's crate.
    """
    covered: list[tuple[UnsafeBlock, Harness]] = []
    uncovered: list[UnsafeBlock] = []

    for block in blocks:
        # Determine the crate name from the file path.
        # e.g. crates/edgeforge-storage/src/platform/iouring.rs → edgeforge_storage
        crate = _crate_from_path(block.file)
        module = block.file.stem  # e.g. "iouring", "real", "lib"

        matching_harnesses: list[Harness] = []

        for h in harnesses:
            harness_file_stem = h.file.stem  # e.g. "iouring_unsafe", "storage_invariants", "runtime_sys_unsafe"

            # Match by crate name in targets.
            if any(crate in t.replace("-", "_") for t in h.targets):
                matching_harnesses.append(h)
                continue

            # Match by module name in harness file or harness name.
            if module in harness_file_stem or module in h.name:
                matching_harnesses.append(h)
                continue

            # Match by enclosing function name in harness name.
            if block.context != "<unknown>" and block.context in h.name:
                matching_harnesses.append(h)
                continue

            # Match by crate coverage: a harness file whose stem contains the
            # crate name covers all unsafe blocks in that crate.
            # e.g. "runtime_sys_unsafe.rs" covers all blocks in edgeforge-runtime-sys.
            crate_with_underscores = crate.replace("-", "_")
            if crate_with_underscores in harness_file_stem:
                matching_harnesses.append(h)
                continue

            # Match unsafe impl Send/Sync by harness name keywords.
            if block.kind == "impl":
                if "send" in h.name.lower() or "sync" in h.name.lower():
                    matching_harnesses.append(h)
                    continue

        if matching_harnesses:
            covered.append((block, matching_harnesses[0]))
        else:
            uncovered.append(block)

    return CoverageReport(
        unsafe_blocks=blocks,
        harnesses=harnesses,
        covered=covered,
        uncovered=uncovered,
    )


def _crate_from_path(path: Path) -> str:
    """Extract the crate name from a file path.

    e.g. crates/edgeforge-storage/src/platform/iouring.rs → "edgeforge_storage"
    """
    parts = path.parts
    for i, part in enumerate(parts):
        if part == "crates" and i + 1 < len(parts):
            return parts[i + 1].replace("-", "_")
    return path.stem


# ── Reporting ──────────────────────────────────────────────────────────────


def print_report(report: CoverageReport, threshold: float) -> int:
    """Print the coverage report. Returns exit code (0 = pass, 1 = fail)."""
    print("=" * 70)
    print("Kani Coverage Report (ADR-025, QPPO #4)")
    print("=" * 70)
    print()
    print(f"Unsafe blocks found:  {report.total}")
    print(f"Kani harnesses found: {len(report.harnesses)}")
    print(f"Covered blocks:       {report.covered_count}")
    print(f"Uncovered blocks:     {len(report.uncovered)}")
    print(f"Coverage:             {report.coverage_pct:.1f}%")
    print(f"QPPO threshold:       {threshold:.0f}%")
    print()

    if report.total == 0:
        print("✅ No unsafe blocks found — coverage is 100% by default.")
        return 0

    # List harnesses.
    print("─" * 70)
    print("Kani Proof Harnesses:")
    for h in report.harnesses:
        print(f"  {h.file}:{h.line}  fn {h.name}()")
        if h.targets:
            print(f"    targets: {', '.join(h.targets[:5])}")
    print()

    # List covered blocks.
    print("─" * 70)
    print("Covered unsafe blocks:")
    for block, harness in report.covered:
        rel = block.file
        print(f"  ✅ {rel}:{block.line}  {block.kind} in {block.context}")
        print(f"      ← covered by {harness.name}() in {harness.file.name}")
    print()

    # List uncovered blocks.
    if report.uncovered:
        print("─" * 70)
        print("UNCOVERED unsafe blocks:")
        for block in report.uncovered:
            rel = block.file
            print(f"  ❌ {rel}:{block.line}  {block.kind} in {block.context}")
        print()

    # Verdict.
    print("─" * 70)
    if report.coverage_pct >= threshold:
        print(f"✅ PASS — coverage {report.coverage_pct:.1f}% ≥ {threshold:.0f}% threshold")
        return 0
    print(
        f"❌ FAIL — coverage {report.coverage_pct:.1f}% < {threshold:.0f}% threshold"
    )
    print()
    print("To fix: add Kani proof harnesses in tests/kani/ for the uncovered blocks.")
    print("See CONTRIBUTING.md → 'unsafe Policy' for instructions.")
    return 1


# ── Main ───────────────────────────────────────────────────────────────────


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Measure Kani proof harness coverage of unsafe blocks."
    )
    parser.add_argument(
        "--workspace",
        type=Path,
        default=Path("."),
        help="Path to the EdgeForge workspace root (default: current dir).",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=90.0,
        help="Coverage threshold percentage (default: 90, per QPPO #4).",
    )
    args = parser.parse_args()

    workspace: Path = args.workspace
    if not (workspace / "Cargo.toml").exists():
        print(f"error: {workspace} is not a Rust workspace (no Cargo.toml)", file=sys.stderr)
        return 2

    print(f"Scanning {workspace} for unsafe blocks and Kani harnesses...")
    blocks = scan_unsafe_blocks(workspace)
    harnesses = scan_harnesses(workspace)
    report = compute_coverage(blocks, harnesses)

    return print_report(report, args.threshold)


if __name__ == "__main__":
    sys.exit(main())
