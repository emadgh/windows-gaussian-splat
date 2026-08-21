#!/usr/bin/env python3
"""Download only inference assets needed by Gaussian Splat Studio.

Authentication is delegated to `hf auth login`; tokens are never accepted as
command-line arguments or written by this application.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path

from huggingface_hub import snapshot_download

ROOT = Path(os.environ.get("GSS_ROOT", "/opt/gss"))
HARMONIZER = ROOT / "upstream" / "harmonizer"


def event(stage: str, progress: float, message: str) -> None:
    print(
        "GSS_EVENT "
        + json.dumps({"stage": stage, "progress": progress, "message": message}),
        flush=True,
    )


def require_checkout() -> None:
    if not (HARMONIZER / "download_checkpoints.sh").exists():
        raise SystemExit(
            f"Harmonizer checkout not found at {HARMONIZER}. Run runtime setup first."
        )


def download_recommended() -> None:
    require_checkout()
    harmonizer_dir = HARMONIZER / "models"
    cosmos_dir = (
        HARMONIZER
        / "src"
        / "checkpoints"
        / "nvidia"
        / "Cosmos-Predict2-0.6B-Text2Image"
    )
    harmonizer_dir.mkdir(parents=True, exist_ok=True)
    cosmos_dir.mkdir(parents=True, exist_ok=True)

    event("models", 0.05, "Downloading NVIDIA Harmonizer paper checkpoint (~5.04 GB)")
    snapshot_download(
        repo_id="nvidia/Harmonizer",
        local_dir=harmonizer_dir,
        allow_patterns=["diffusion_harmonizer.pkl"],
    )

    event("models", 0.58, "Downloading Cosmos Predict2 0.6B base model and tokenizer")
    snapshot_download(
        repo_id="nvidia/Cosmos-Predict2-0.6B-Text2Image",
        local_dir=cosmos_dir,
        allow_patterns=[
            "model.pt",
            "config.json",
            "*.json",
            "*.yaml",
            "tokenizer/**",
            "tokenizer_fast/**",
        ],
    )

    checkpoint = harmonizer_dir / "diffusion_harmonizer.pkl"
    cosmos_model = cosmos_dir / "model.pt"
    if not checkpoint.exists() or not cosmos_model.exists():
        raise SystemExit("Model download completed but required checkpoint files are missing")

    event("models", 1.0, "Recommended local Harmonizer model pack is ready")


def download_full() -> None:
    require_checkout()
    event("models", 0.05, "Downloading the full official Harmonizer checkpoint repository")
    snapshot_download(repo_id="nvidia/Harmonizer", local_dir=HARMONIZER / "models")
    event("models", 0.50, "Downloading the full official Cosmos Predict2 0.6B repository")
    snapshot_download(
        repo_id="nvidia/Cosmos-Predict2-0.6B-Text2Image",
        local_dir=(
            HARMONIZER
            / "src"
            / "checkpoints"
            / "nvidia"
            / "Cosmos-Predict2-0.6B-Text2Image"
        ),
    )
    event("models", 1.0, "Full Harmonizer model pack is ready")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pack", choices=["recommended", "full"], default="recommended")
    args = parser.parse_args()

    try:
        if args.pack == "recommended":
            download_recommended()
        else:
            download_full()
    except Exception as exc:  # provide an actionable auth/license error in the desktop log
        event(
            "models",
            0.0,
            "Download failed. Run 'hf auth login' in the GSS WSL runtime and accept the "
            "Cosmos-Predict2 model terms on Hugging Face, then retry.",
        )
        raise SystemExit(str(exc)) from exc


if __name__ == "__main__":
    main()
