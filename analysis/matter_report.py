"""Read recorded material observations without running the simulator."""
import argparse
import json
from pathlib import Path

import numpy as np
import polars as pl


def report(directory: Path) -> dict:
    with (directory / "manifest.json").open(encoding="utf-8") as stream:
        manifest = json.load(stream)
    with (directory / "outcome.json").open(encoding="utf-8") as stream:
        outcome = json.load(stream)
    rows = []
    with (directory / "history.ndjson").open(encoding="utf-8") as stream:
        for line in stream:
            row = json.loads(line)
            if row["generation"] > outcome["final_generation"]:
                break
            metrics = row.get("metrics")
            if metrics is None:
                continue
            sizes = metrics.get("size_distribution", {})
            counts = np.array([int(n) for n in sizes.values()], dtype=np.float64)
            probabilities = counts / counts.sum() if counts.size and counts.sum() else counts
            entropy = float(-(probabilities * np.log2(probabilities)).sum()) if probabilities.size else 0.0
            rows.append({
                "generation": row["generation"],
                "bonds": metrics.get("bonds", 0),
                "components": metrics.get("components", 0),
                "largest_component": metrics.get("largest_component", 0),
                "active_conformations": metrics.get("active_conformations", 0),
                "component_size_entropy": entropy,
                "events_truncated": metrics.get("events_truncated", False),
                "total_energy": metrics.get("total_energy", "0"),
            })
    table = pl.DataFrame(rows)
    return {
        "manifest": manifest,
        "outcome": outcome,
        "observations": table.to_dicts(),
        "sampled_observations_report_no_truncation": not any(row["events_truncated"] for row in rows),
        "provenance_note": "Unsampled event batches must also be inspected to establish complete retention.",
        "interpretation": "Size entropy and conformation persistence do not establish life, functional memory, learning, or open-ended evolution.",
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    result = report(args.directory)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("x", encoding="utf-8") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")


if __name__ == "__main__":
    main()
