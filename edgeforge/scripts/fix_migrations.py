#!/usr/bin/env python3
"""
fix_migrations.py — Repair the 16 EdgeForge SQL migration files.

Problem: every file in resources/migrations/ contains the literal two-character
sequence '\\n' (backslash + n) instead of a real newline (0x0A). Because each
file begins with `-- <filename>`, the leading SQL comment swallows the entire
file when fed to SQLite, so sqlx::migrate!() creates an empty schema and the
first INSERT fails.

This script:
  1. Reads each .sql file as bytes.
  2. Replaces the literal b'\\n' (0x5C 0x6E) with b'\\n' (0x0A).
  3. Writes the file back in place.
  4. Re-reads and validates: file must contain real newlines, must end with
     a newline, and must have no remaining literal '\\n' sequences.

Usage:
    python3 fix_migrations.py <migrations_dir>
"""

from __future__ import annotations

import sys
from pathlib import Path


def fix_file(path: Path) -> tuple[bool, str]:
    """Repair a single migration file. Returns (ok, message)."""
    raw = path.read_bytes()
    if b"\n" in raw and b"\\n" not in raw:
        # Already has real newlines and no literal \n — leave it alone.
        return True, f"OK (already fixed): {path.name}"

    if b"\\n" not in raw:
        return False, f"FAIL (no literal \\n and no real newline?): {path.name}"

    # Replace literal backslash-n with real newline.
    fixed = raw.replace(b"\\n", b"\n")

    # Guarantee trailing newline.
    if not fixed.endswith(b"\n"):
        fixed += b"\n"

    path.write_bytes(fixed)

    # Validate.
    re_read = path.read_bytes()
    if b"\\n" in re_read:
        return False, f"FAIL (literal \\n still present after fix): {path.name}"
    if b"\n" not in re_read:
        return False, f"FAIL (no real newline after fix): {path.name}"

    line_count = re_read.count(b"\n")
    return True, f"FIXED: {path.name} ({line_count} lines)"


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <migrations_dir>", file=sys.stderr)
        return 2

    migrations_dir = Path(sys.argv[1])
    if not migrations_dir.is_dir():
        print(f"error: {migrations_dir} is not a directory", file=sys.stderr)
        return 2

    sql_files = sorted(migrations_dir.glob("*.sql"))
    if not sql_files:
        print(f"error: no .sql files in {migrations_dir}", file=sys.stderr)
        return 2

    print(f"Fixing {len(sql_files)} migration files in {migrations_dir} ...")
    failures = 0
    for f in sql_files:
        ok, msg = fix_file(f)
        print(f"  {msg}")
        if not ok:
            failures += 1

    if failures:
        print(f"\nFAILED: {failures} file(s) could not be fixed.")
        return 1

    print(f"\nAll {len(sql_files)} migration files fixed successfully.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
