"""Rebuild the fixed original reactor in D, preserving historical missing evidence paths."""
from pathlib import Path
from build import run
ROOT=Path(__file__).resolve().parents[2]
if __name__=="__main__":
    result=run(ROOT/"docs/phase-2/evidence/source-junit-rebuild")
    print("REBUILT_ORIGINAL_SOURCE "+str(result),flush=True)
