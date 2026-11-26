"""Authenticate the rebuilt local Maven inputs against exact published Central checksums."""
from pathlib import Path
from authenticate import run
ROOT=Path(__file__).resolve().parents[2]
if __name__=="__main__":
    run(ROOT/"docs/phase-2/evidence/source-junit-rebuild")
