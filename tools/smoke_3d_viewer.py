#!/usr/bin/env python3
"""Headless smoke test for the embedded 3D viewer.

Runs the same end-to-end pipeline the in-app widget uses — parse a
Bully NIF, upload geometry + texture to wgpu, render one frame — and
writes the result as a PNG. Skips silently when the fixture path is
missing on the test machine.

Usage::

    python tools/smoke_3d_viewer.py [output_png]

Default output: target/scene3d-bully-1950fridge.png.
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

FIXTURE = Path(
    "C:/Games/Bully - Scholarship Edition/Stream/test1/1950Fridge.nif"
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "output",
        nargs="?",
        default=str(Path(__file__).resolve().parent.parent / "target" / "scene3d-bully-1950fridge.png"),
        help="PNG path to write (default: target/scene3d-bully-1950fridge.png)",
    )
    args = parser.parse_args()

    if not FIXTURE.exists():
        print(f"[smoke] fixture not present at {FIXTURE}; skipping", file=sys.stderr)
        return 0

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    if out_path.exists():
        out_path.unlink()

    env = dict(os.environ)
    repo_root = Path(__file__).resolve().parent.parent
    # Use --debug rather than --release: the release profile trips a
    # known stack-overflow in unrelated image-codec crates
    # (rav1e/read-fonts) on this toolchain.
    cmd = [
        "cargo",
        "test",
        "--lib",
        "bully_fixture_renders_to_png_when_present",
        "--",
        "--nocapture",
    ]
    print(f"[smoke] running {' '.join(cmd)} in {repo_root}")
    res = subprocess.run(cmd, cwd=str(repo_root), env=env)
    if res.returncode != 0:
        print("[smoke] cargo test failed", file=sys.stderr)
        return res.returncode

    candidate = (
        repo_root
        / "target"
        / "scene3d-bully-1950fridge.png"
    )
    if not candidate.exists():
        print(f"[smoke] expected PNG at {candidate} but it was not produced", file=sys.stderr)
        return 1

    if candidate.resolve() != out_path.resolve():
        out_path.write_bytes(candidate.read_bytes())
        print(f"[smoke] copied {candidate} -> {out_path}")
    else:
        print(f"[smoke] wrote {out_path}")
    print(f"[smoke] ok ({out_path.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
