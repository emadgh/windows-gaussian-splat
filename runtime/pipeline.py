#!/usr/bin/env python3
"""End-to-end local reconstruction pipeline used by the Rust desktop app."""

from __future__ import annotations

import argparse
import json
import math
import os
import shutil
import subprocess
import sys
from pathlib import Path

import cv2
import numpy as np

GSS_ROOT = Path(os.environ.get("GSS_ROOT", "/opt/gss"))
GSPLAT = GSS_ROOT / "upstream" / "gsplat"
HARMONIZER = GSS_ROOT / "upstream" / "harmonizer"
PYTHON = GSS_ROOT / "venv" / "bin" / "python"
REFINE = Path(__file__).with_name("refine.py")


def event(stage: str, progress: float, message: str) -> None:
    print(
        "GSS_EVENT "
        + json.dumps({"stage": stage, "progress": progress, "message": message}),
        flush=True,
    )


def run(cmd: list[str], cwd: Path | None = None, env: dict[str, str] | None = None) -> None:
    printable = " ".join(str(value) for value in cmd)
    print(f"[GSS] $ {printable}", flush=True)
    process = subprocess.Popen(
        [str(value) for value in cmd],
        cwd=str(cwd) if cwd else None,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )
    assert process.stdout is not None
    for line in process.stdout:
        print(line.rstrip(), flush=True)
    code = process.wait()
    if code != 0:
        raise RuntimeError(f"Command failed with exit code {code}: {printable}")


def command_help(command: list[str]) -> str:
    result = subprocess.run(command + ["-h"], capture_output=True, text=True, check=False)
    return (result.stdout or "") + "\n" + (result.stderr or "")


def extract_frames(video: Path, raw_dir: Path, fps: float, force: bool) -> None:
    if force and raw_dir.exists():
        shutil.rmtree(raw_dir)
    raw_dir.mkdir(parents=True, exist_ok=True)
    existing = list(raw_dir.glob("frame_*.jpg"))
    if existing:
        event("frames", 0.12, f"Reusing {len(existing)} previously extracted frames")
        return

    run(
        [
            "ffmpeg",
            "-hide_banner",
            "-loglevel",
            "warning",
            "-i",
            str(video),
            "-vf",
            f"fps={fps:g}",
            "-q:v",
            "2",
            str(raw_dir / "frame_%06d.jpg"),
        ]
    )


def frame_signature(gray: np.ndarray) -> np.ndarray:
    tiny = cv2.resize(gray, (64, 36), interpolation=cv2.INTER_AREA).astype(np.float32)
    tiny -= tiny.mean()
    norm = float(np.linalg.norm(tiny))
    if norm > 1e-6:
        tiny /= norm
    return tiny.reshape(-1)


def filter_frames(
    raw_dir: Path,
    images_dir: Path,
    blur_threshold: float,
    duplicate_threshold: float,
    max_frames: int,
    force: bool,
) -> int:
    if force and images_dir.exists():
        shutil.rmtree(images_dir)
    images_dir.mkdir(parents=True, exist_ok=True)
    existing = sorted(images_dir.glob("*.jpg"))
    if existing:
        return len(existing)

    candidates: list[tuple[Path, float, np.ndarray]] = []
    for path in sorted(raw_dir.glob("frame_*.jpg")):
        gray = cv2.imread(str(path), cv2.IMREAD_GRAYSCALE)
        if gray is None:
            continue
        sharpness = float(cv2.Laplacian(gray, cv2.CV_64F).var())
        candidates.append((path, sharpness, frame_signature(gray)))

    if not candidates:
        raise RuntimeError("FFmpeg produced no readable frames")

    accepted: list[tuple[Path, float, np.ndarray]] = []
    previous_signature: np.ndarray | None = None
    for item in candidates:
        path, sharpness, signature = item
        if sharpness < blur_threshold:
            continue
        similarity = (
            float(np.dot(previous_signature, signature))
            if previous_signature is not None
            else -1.0
        )
        if similarity >= duplicate_threshold:
            continue
        accepted.append(item)
        previous_signature = signature
        if len(accepted) >= max_frames:
            break

    minimum = min(24, len(candidates))
    if len(accepted) < minimum:
        event(
            "frames",
            0.16,
            "Capture is blur/duplicate heavy; keeping the sharpest fallback frames instead of failing",
        )
        sharpest = sorted(candidates, key=lambda item: item[1], reverse=True)[: max(minimum, len(accepted))]
        accepted = sorted(sharpest, key=lambda item: item[0].name)[:max_frames]

    for index, (source, _, _) in enumerate(accepted, start=1):
        shutil.copy2(source, images_dir / f"frame_{index:06d}.jpg")

    return len(accepted)


def colmap_gpu_option(subcommand: str, current: str, legacy: str) -> str:
    help_text = command_help(["colmap", subcommand])
    if current in help_text:
        return current
    if legacy in help_text:
        return legacy
    return current


def solve_colmap(data_dir: Path, frame_count: int, matcher: str, force: bool) -> Path:
    database = data_dir / "database.db"
    images = data_dir / "images"
    sparse = data_dir / "sparse"
    model = sparse / "0"

    if model.exists() and any(model.iterdir()) and not force:
        event("cameras", 0.35, "Reusing existing COLMAP camera solution")
        return model

    if force:
        if database.exists():
            database.unlink()
        if sparse.exists():
            shutil.rmtree(sparse)
    sparse.mkdir(parents=True, exist_ok=True)

    feature_gpu = colmap_gpu_option(
        "feature_extractor", "--FeatureExtraction.use_gpu", "--SiftExtraction.use_gpu"
    )
    run(
        [
            "colmap",
            "feature_extractor",
            "--database_path",
            str(database),
            "--image_path",
            str(images),
            "--ImageReader.single_camera",
            "1",
            "--ImageReader.camera_model",
            "OPENCV",
            feature_gpu,
            "1",
        ]
    )

    selected = matcher
    if selected == "auto":
        selected = "exhaustive" if frame_count <= 350 else "sequential"
    matcher_command = f"{selected}_matcher"
    match_gpu = colmap_gpu_option(
        matcher_command, "--FeatureMatching.use_gpu", "--SiftMatching.use_gpu"
    )
    matcher_cmd = [
        "colmap",
        matcher_command,
        "--database_path",
        str(database),
        match_gpu,
        "1",
    ]
    if selected == "sequential":
        help_text = command_help(["colmap", matcher_command])
        if "--SequentialMatching.overlap" in help_text:
            matcher_cmd += ["--SequentialMatching.overlap", "20"]
        elif "--SequentialMatching.overlap" not in help_text and "--SequentialMatching.loop_detection" in help_text:
            matcher_cmd += ["--SequentialMatching.loop_detection", "0"]
    run(matcher_cmd)

    run(
        [
            "colmap",
            "mapper",
            "--database_path",
            str(database),
            "--image_path",
            str(images),
            "--output_path",
            str(sparse),
        ]
    )

    models = sorted(path for path in sparse.iterdir() if path.is_dir())
    if not models:
        raise RuntimeError("COLMAP could not register a camera model from this capture")
    if model not in models:
        model = models[0]
    return model


def train_3dgut(data_dir: Path, result_dir: Path, max_steps: int, cap_max: int, force: bool) -> Path:
    ply_dir = result_dir / "ply"
    if not force and ply_dir.exists():
        existing = sorted(ply_dir.glob("*.ply"), key=lambda path: path.stat().st_mtime)
        if existing:
            event("training", 0.62, "Reusing completed raw 3DGUT PLY")
            return existing[-1]

    if force and result_dir.exists():
        shutil.rmtree(result_dir)
    result_dir.mkdir(parents=True, exist_ok=True)

    trainer = GSPLAT / "examples" / "simple_trainer.py"
    if not trainer.exists():
        raise RuntimeError("gsplat example trainer is missing; run runtime setup/repair")

    env = os.environ.copy()
    env.setdefault("CUDA_HOME", "/usr/local/cuda-12.8")
    env["PATH"] = f"/usr/local/cuda-12.8/bin:{env.get('PATH', '')}"
    run(
        [
            str(PYTHON),
            str(trainer),
            "mcmc",
            "--data-dir",
            str(data_dir),
            "--data-factor",
            "1",
            "--result-dir",
            str(result_dir),
            "--disable-viewer",
            "--disable-video",
            "--with-ut",
            "--with-eval3d",
            "--save-ply",
            "--no-normalize-world-space",
            "--max-steps",
            str(max_steps),
            "--eval-steps",
            str(max_steps),
            "--save-steps",
            str(max_steps),
            "--ply-steps",
            str(max_steps),
            "--strategy.cap-max",
            str(cap_max),
        ],
        cwd=GSPLAT / "examples",
        env=env,
    )

    candidates = sorted(ply_dir.glob("*.ply"), key=lambda path: path.stat().st_mtime)
    if not candidates:
        raise RuntimeError("gsplat training finished without producing a PLY")
    return candidates[-1]


def harmonize(raw_views: Path, temporal: bool, resolution: int) -> Path:
    checkpoint = HARMONIZER / "models" / "diffusion_harmonizer.pkl"
    cosmos = (
        HARMONIZER
        / "src"
        / "checkpoints"
        / "nvidia"
        / "Cosmos-Predict2-0.6B-Text2Image"
        / "model.pt"
    )
    if not checkpoint.exists() or not cosmos.exists():
        raise RuntimeError(
            "Harmonizer models are missing. Authenticate Hugging Face and install the recommended model pack."
        )

    project_root = raw_views.parents[2]
    relative_raw = raw_views.relative_to(project_root)
    container_raw = Path("/project") / relative_raw
    output = raw_views.with_name(raw_views.name + "_harmonized")
    if output.exists():
        shutil.rmtree(output)

    base = [
        "docker",
        "run",
        "--rm",
        "--gpus",
        "all",
        "--shm-size=8g",
        "-v",
        f"{HARMONIZER}:/work",
        "-v",
        f"{project_root}:/project",
        "-w",
        "/work/src",
        "gss-harmonizer:latest",
        "python",
        "inference_pix2pix_turbo_harmonizer.py",
        "--input_image",
        str(container_raw),
        "--model_path",
        "/work/models/diffusion_harmonizer.pkl",
        "--model_identifier",
        "harmonized",
        "--timestep",
        "250",
        "--resolution",
        str(resolution),
        "--use_sched",
    ]
    cmd = base if temporal else base + ["--nontemporal"]
    try:
        run(cmd)
    except RuntimeError:
        if not temporal:
            raise
        event("harmonizer", 0.73, "Temporal Harmonizer failed; retrying in 16 GB non-temporal mode")
        run(base + ["--nontemporal"])

    if not output.exists():
        raise RuntimeError(f"Harmonizer output folder was not created: {output}")
    return output


def refine_round(
    current_ply: Path,
    data_dir: Path,
    project_root: Path,
    round_index: int,
    temporal: bool,
    resolution: int,
    distill_steps: int,
    max_mean_change: float,
    min_edge_correlation: float,
) -> tuple[Path, Path]:
    round_dir = project_root / "refinement" / f"round_{round_index:02d}"
    raw_views = round_dir / "raw"
    camera_json = round_dir / "cameras.json"
    refined_ply = round_dir / "refined.ply"
    report = round_dir / "report.json"
    round_dir.mkdir(parents=True, exist_ok=True)

    run(
        [
            str(PYTHON),
            str(REFINE),
            "render",
            "--input-ply",
            str(current_ply),
            "--data-dir",
            str(data_dir),
            "--output-dir",
            str(raw_views),
            "--camera-json",
            str(camera_json),
            "--resolution",
            str(resolution),
            "--max-views",
            "48",
        ]
    )
    harmonized = harmonize(raw_views, temporal=temporal, resolution=resolution)
    run(
        [
            str(PYTHON),
            str(REFINE),
            "distill",
            "--input-ply",
            str(current_ply),
            "--raw-dir",
            str(raw_views),
            "--harmonized-dir",
            str(harmonized),
            "--camera-json",
            str(camera_json),
            "--output-ply",
            str(refined_ply),
            "--report",
            str(report),
            "--steps",
            str(distill_steps),
            "--max-mean-change",
            str(max_mean_change),
            "--min-edge-correlation",
            str(min_edge_correlation),
        ]
    )
    return refined_ply, report


def export_spz(ply: Path, spz_path: Path) -> bool:
    script = r'''
import spz, sys
src, dst = sys.argv[1], sys.argv[2]
unpack = spz.UnpackOptions()
unpack.to_coord = spz.CoordinateSystem.RDF
cloud = spz.load_splat_from_ply(src, unpack)
pack = spz.PackOptions()
pack.from_coord = spz.CoordinateSystem.RDF
spz.save_spz(cloud, pack, dst)
'''
    result = subprocess.run([str(PYTHON), "-c", script, str(ply), str(spz_path)], text=True)
    return result.returncode == 0 and spz_path.exists()


def write_manifest(args: argparse.Namespace, raw_ply: Path, refined_ply: Path, spz: Path | None, reports: list[Path]) -> None:
    manifest_path = args.output_dir / "project.json"
    manifest: dict[str, object] = {}
    if manifest_path.exists():
        try:
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            manifest = {}
    manifest.update(
        {
            "schema_version": 1,
            "status": "completed",
            "source_video": str(args.video),
            "output_dir": str(args.output_dir),
            "coordinate_system": {
                "source": "COLMAP / gsplat world space",
                "unit": args.unit,
                "up_axis": args.up_axis,
                "scale_multiplier": args.scale_multiplier,
                "note": "PLY is kept in reconstruction coordinates; set/verify asset orientation in 3ds Max.",
            },
            "outputs": {
                "raw_ply": str(raw_ply),
                "refined_ply": str(refined_ply),
                "refined_spz": str(spz) if spz else None,
                "refinement_reports": [str(path) for path in reports],
            },
        }
    )
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--video", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--frame-fps", type=float, default=3.0)
    parser.add_argument("--blur-threshold", type=float, default=70.0)
    parser.add_argument("--duplicate-threshold", type=float, default=0.985)
    parser.add_argument("--max-frames", type=int, default=1200)
    parser.add_argument("--matcher", choices=["auto", "exhaustive", "sequential"], default="auto")
    parser.add_argument("--max-steps", type=int, default=30000)
    parser.add_argument("--cap-max", type=int, default=1_000_000)
    parser.add_argument("--refinement-rounds", type=int, choices=range(0, 4), default=1)
    parser.add_argument("--temporal", action="store_true")
    parser.add_argument("--resolution", type=int, choices=[960, 1024, 1360], default=1024)
    parser.add_argument("--distill-steps", type=int, default=750)
    parser.add_argument("--max-mean-change", type=float, default=0.22)
    parser.add_argument("--min-edge-correlation", type=float, default=0.35)
    parser.add_argument("--export-spz", action="store_true")
    parser.add_argument("--unit", default="millimeter")
    parser.add_argument("--up-axis", default="Z")
    parser.add_argument("--scale-multiplier", type=float, default=1.0)
    parser.add_argument("--force", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    args.video = args.video.expanduser().resolve()
    args.output_dir = args.output_dir.expanduser().resolve()
    if not args.video.exists():
        raise SystemExit(f"Input video does not exist: {args.video}")
    if args.scale_multiplier <= 0 or not math.isfinite(args.scale_multiplier):
        raise SystemExit("Scale multiplier must be a positive finite number")

    args.output_dir.mkdir(parents=True, exist_ok=True)
    data_dir = args.output_dir / "data"
    raw_frames = data_dir / "images_raw"
    images = data_dir / "images"

    event("prepare", 0.01, "Preparing project workspace")
    extract_frames(args.video, raw_frames, args.frame_fps, args.force)
    frame_count = filter_frames(
        raw_frames,
        images,
        args.blur_threshold,
        args.duplicate_threshold,
        args.max_frames,
        args.force,
    )
    event("frames", 0.20, f"Selected {frame_count} sharp, non-duplicate frames")

    solve_colmap(data_dir, frame_count, args.matcher, args.force)
    event("cameras", 0.42, "COLMAP camera solve complete")

    trained_ply = train_3dgut(
        data_dir,
        args.output_dir / "training_raw",
        args.max_steps,
        args.cap_max,
        args.force,
    )
    raw_ply = args.output_dir / "raw.ply"
    shutil.copy2(trained_ply, raw_ply)
    event("training", 0.66, "Raw 3DGUT reconstruction exported")

    current = raw_ply
    reports: list[Path] = []
    if args.refinement_rounds:
        for round_index in range(1, args.refinement_rounds + 1):
            start = 0.66 + (round_index - 1) * (0.24 / args.refinement_rounds)
            event(
                "harmonizer",
                start,
                f"Rendering conservative pseudo-views for refinement round {round_index}",
            )
            current, report = refine_round(
                current,
                data_dir,
                args.output_dir,
                round_index,
                args.temporal,
                args.resolution,
                args.distill_steps,
                args.max_mean_change,
                args.min_edge_correlation,
            )
            reports.append(report)
            event(
                "refinement",
                0.66 + round_index * (0.24 / args.refinement_rounds),
                f"Refinement round {round_index} distilled into the 3D splat",
            )

    final_ply = args.output_dir / "refined.ply"
    if args.scale_multiplier != 1.0:
        run(
            [
                str(PYTHON),
                str(REFINE),
                "scale",
                "--input-ply",
                str(current),
                "--output-ply",
                str(final_ply),
                "--factor",
                str(args.scale_multiplier),
            ]
        )
    else:
        shutil.copy2(current, final_ply)

    spz_path: Path | None = None
    if args.export_spz:
        candidate = args.output_dir / "refined.spz"
        if export_spz(final_ply, candidate):
            spz_path = candidate
        else:
            event("export", 0.95, "SPZ conversion failed; PLY remains valid and is the primary 3ds Max asset")

    write_manifest(args, raw_ply, final_ply, spz_path, reports)
    event("export", 1.0, f"Completed: {final_ply}")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        event("cancelled", 0.0, "Pipeline cancelled")
        raise SystemExit(130)
    except Exception as exc:
        event("failed", 0.0, str(exc))
        raise
