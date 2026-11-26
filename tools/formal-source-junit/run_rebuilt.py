"""Execute the rebuilt original modules without writing into unavailable historical junctions."""
import argparse
from pathlib import Path
from run import run
ROOT=Path(__file__).resolve().parents[2]
if __name__=="__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("--build",type=Path,required=True)
    args=parser.parse_args()
    run(args.build,"execute",ROOT/"docs/phase-2/evidence/source-junit-rebuild")
