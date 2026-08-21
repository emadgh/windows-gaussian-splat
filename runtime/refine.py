#!/usr/bin/env python3
"""Conservative offline Harmonizer refinement for an existing 3D Gaussian Splat.

Pseudo views are interpolated only between registered COLMAP cameras. Harmonizer
outputs are gated before being used as supervision. Distillation updates the
actual splat parameters with explicit geometry anchors and no densification.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import sys
from pathlib import Path

import cv2
import numpy as np
import torch
import torch.nn.functional as F
from PIL import Image
from scipy.spatial.transform import Rotation, Slerp

GSS_ROOT = Path(os.environ.get("GSS_ROOT", "/opt/gss"))
GSPLAT = GSS_ROOT / "upstream" / "gsplat"
sys.path.insert(0, str(GSPLAT / "examples"))

from datasets.colmap import Parser  # noqa: E402
from gsplat.exporter import export_splats, load_ply_to_splats  # noqa: E402
from gsplat.rendering import rasterization  # noqa: E402

RESOLUTION_MAP = {1024: (1024, 576), 960: (960, 544), 1360: (1360, 768)}


def event(stage: str, progress: float, message: str) -> None:
    print(
        "GSS_EVENT "
        + json.dumps({"stage": stage, "progress": progress, "message": message}),
        flush=True,
    )


def sh_degree(sh0: torch.Tensor, shn: torch.Tensor) -> int:
    count = sh0.shape[1] + shn.shape[1]
    degree = int(round(math.sqrt(count) - 1))
    if (degree + 1) ** 2 != count:
        raise ValueError(f"Invalid SH basis count: {count}")
    return degree


def activated(splats: dict[str, torch.Tensor]) -> tuple[torch.Tensor, ...]:
    colors = torch.cat([splats["sh0"], splats["shN"]], dim=1)
    return (
        splats["means"],
        F.normalize(splats["quats"], dim=-1),
        splats["scales"].exp(),
        splats["opacities"].sigmoid(),
        colors,
    )


def render_splats(
    splats: dict[str, torch.Tensor],
    c2w: np.ndarray,
    K: np.ndarray,
    width: int,
    height: int,
) -> torch.Tensor:
    means, quats, scales, opacities, colors = activated(splats)
    device = means.device
    c2w_t = torch.from_numpy(c2w).float().to(device)
    viewmat = torch.linalg.inv(c2w_t)[None]
    K_t = torch.from_numpy(K).float().to(device)[None]
    degree = sh_degree(splats["sh0"], splats["shN"])
    rendered, _, _ = rasterization(
        means,
        quats,
        scales,
        opacities,
        colors,
        viewmat,
        K_t,
        width,
        height,
        sh_degree=degree,
        packed=False,
        render_mode="RGB",
        camera_model="pinhole",
        with_ut=True,
        with_eval3d=True,
    )
    return rendered[0, ..., :3].clamp(0.0, 1.0)


def midpoint_pose(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    rotations = Rotation.from_matrix(np.stack([a[:3, :3], b[:3, :3]], axis=0))
    rotation = Slerp([0.0, 1.0], rotations)([0.5]).as_matrix()[0]
    result = np.eye(4, dtype=np.float32)
    result[:3, :3] = rotation.astype(np.float32)
    result[:3, 3] = ((a[:3, 3] + b[:3, 3]) * 0.5).astype(np.float32)
    return result


def scaled_intrinsics(K: np.ndarray, source_size: tuple[int, int], target_size: tuple[int, int]) -> np.ndarray:
    source_w, source_h = source_size
    target_w, target_h = target_size
    result = K.astype(np.float32).copy()
    result[0, :] *= target_w / source_w
    result[1, :] *= target_h / source_h
    return result


def choose_supported_pairs(parser: Parser, max_views: int) -> list[tuple[int, int, float]]:
    order = sorted(range(len(parser.image_names)), key=lambda index: parser.image_names[index])
    candidates: list[tuple[int, int, float]] = []
    for left, right in zip(order, order[1:]):
        a = parser.camtoworlds[left, :3, 3]
        b = parser.camtoworlds[right, :3, 3]
        distance = float(np.linalg.norm(a - b))
        if math.isfinite(distance) and distance > 1e-8:
            candidates.append((left, right, distance))

    if not candidates:
        raise RuntimeError("Not enough neighboring registered cameras for conservative pseudo views")

    distances = np.asarray([item[2] for item in candidates], dtype=np.float64)
    median = float(np.median(distances))
    limit = max(median * 2.5, float(np.percentile(distances, 65)))
    supported = [item for item in candidates if item[2] <= limit]
    if not supported:
        supported = sorted(candidates, key=lambda item: item[2])[:1]

    if len(supported) <= max_views:
        return supported
    indices = np.linspace(0, len(supported) - 1, max_views, dtype=int)
    return [supported[index] for index in indices]


def render_command(args: argparse.Namespace) -> None:
    if args.resolution not in RESOLUTION_MAP:
        raise SystemExit(f"Unsupported Harmonizer resolution: {args.resolution}")
    args.output_dir.mkdir(parents=True, exist_ok=True)

    parser = Parser(data_dir=str(args.data_dir), factor=1, normalize=False, test_every=8)
    pairs = choose_supported_pairs(parser, args.max_views)
    splats = {key: value.cuda() for key, value in load_ply_to_splats(str(args.input_ply)).items()}
    target_w, target_h = RESOLUTION_MAP[args.resolution]

    views: list[dict[str, object]] = []
    with torch.inference_mode():
        for output_index, (left, right, gap) in enumerate(pairs):
            c2w = midpoint_pose(parser.camtoworlds[left], parser.camtoworlds[right])
            camera_id = parser.camera_ids[left]
            K = parser.Ks_dict[camera_id]
            source_size = parser.imsize_dict[camera_id]
            K_scaled = scaled_intrinsics(K, source_size, (target_w, target_h))
            rgb = render_splats(splats, c2w, K_scaled, target_w, target_h)
            file_name = f"view_{output_index:04d}.png"
            pixels = (rgb.cpu().numpy() * 255.0).round().astype(np.uint8)
            Image.fromarray(pixels).save(args.output_dir / file_name)
            views.append(
                {
                    "file": file_name,
                    "c2w": c2w.tolist(),
                    "K": K_scaled.tolist(),
                    "width": target_w,
                    "height": target_h,
                    "support": {
                        "left_image": parser.image_names[left],
                        "right_image": parser.image_names[right],
                        "camera_gap": gap,
                    },
                }
            )
            if output_index % 4 == 0 or output_index + 1 == len(pairs):
                event(
                    "pseudo_views",
                    (output_index + 1) / len(pairs),
                    f"Rendered supported pseudo view {output_index + 1}/{len(pairs)}",
                )

    args.camera_json.parent.mkdir(parents=True, exist_ok=True)
    args.camera_json.write_text(
        json.dumps(
            {
                "policy": "midpoint interpolation between adjacent registered COLMAP cameras only",
                "source_ply": str(args.input_ply),
                "views": views,
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    event("pseudo_views", 1.0, f"Created {len(views)} conservative pseudo views")


def image_metrics(raw_path: Path, fixed_path: Path) -> tuple[float, float]:
    raw = cv2.imread(str(raw_path), cv2.IMREAD_COLOR)
    fixed = cv2.imread(str(fixed_path), cv2.IMREAD_COLOR)
    if raw is None or fixed is None:
        return float("inf"), -1.0
    if raw.shape != fixed.shape:
        fixed = cv2.resize(fixed, (raw.shape[1], raw.shape[0]), interpolation=cv2.INTER_AREA)

    raw_f = raw.astype(np.float32) / 255.0
    fixed_f = fixed.astype(np.float32) / 255.0
    mean_change = float(np.mean(np.abs(raw_f - fixed_f)))

    raw_gray = cv2.cvtColor(raw, cv2.COLOR_BGR2GRAY).astype(np.float32)
    fixed_gray = cv2.cvtColor(fixed, cv2.COLOR_BGR2GRAY).astype(np.float32)
    raw_edge = cv2.magnitude(
        cv2.Sobel(raw_gray, cv2.CV_32F, 1, 0), cv2.Sobel(raw_gray, cv2.CV_32F, 0, 1)
    ).reshape(-1)
    fixed_edge = cv2.magnitude(
        cv2.Sobel(fixed_gray, cv2.CV_32F, 1, 0), cv2.Sobel(fixed_gray, cv2.CV_32F, 0, 1)
    ).reshape(-1)
    raw_std = float(raw_edge.std())
    fixed_std = float(fixed_edge.std())
    if raw_std < 1e-6 and fixed_std < 1e-6:
        edge_corr = 1.0
    elif raw_std < 1e-6 or fixed_std < 1e-6:
        edge_corr = 0.0
    else:
        edge_corr = float(np.corrcoef(raw_edge, fixed_edge)[0, 1])
        if not math.isfinite(edge_corr):
            edge_corr = 0.0
    return mean_change, edge_corr


def load_target(path: Path, width: int, height: int, device: torch.device) -> torch.Tensor:
    image = Image.open(path).convert("RGB").resize((width, height), Image.Resampling.LANCZOS)
    array = np.asarray(image, dtype=np.float32) / 255.0
    return torch.from_numpy(array).to(device)


def edge_loss(rendered: torch.Tensor, target: torch.Tensor) -> torch.Tensor:
    render_gray = rendered.mean(dim=-1)
    target_gray = target.mean(dim=-1)
    rx = render_gray[:, 1:] - render_gray[:, :-1]
    tx = target_gray[:, 1:] - target_gray[:, :-1]
    ry = render_gray[1:, :] - render_gray[:-1, :]
    ty = target_gray[1:, :] - target_gray[:-1, :]
    return F.l1_loss(rx, tx) + F.l1_loss(ry, ty)


def psnr_from_mse(mse: float) -> float:
    return -10.0 * math.log10(max(mse, 1e-10))


def distill_command(args: argparse.Namespace) -> None:
    metadata = json.loads(args.camera_json.read_text(encoding="utf-8"))
    view_by_name = {view["file"]: view for view in metadata["views"]}
    accepted: list[dict[str, object]] = []
    rejected: list[dict[str, object]] = []

    for name, view in view_by_name.items():
        raw_path = args.raw_dir / name
        fixed_path = args.harmonized_dir / name
        mean_change, edge_corr = image_metrics(raw_path, fixed_path)
        record = {
            "file": name,
            "mean_change": mean_change,
            "edge_correlation": edge_corr,
            "accepted": mean_change <= args.max_mean_change and edge_corr >= args.min_edge_correlation,
        }
        if record["accepted"]:
            accepted.append({**view, **record, "fixed_path": str(fixed_path)})
        else:
            rejected.append(record)

    report_data: dict[str, object] = {
        "source_ply": str(args.input_ply),
        "policy": {
            "max_mean_change": args.max_mean_change,
            "min_edge_correlation": args.min_edge_correlation,
            "geometry_anchor": True,
            "densification": False,
        },
        "accepted_views": len(accepted),
        "rejected_views": len(rejected),
        "rejected": rejected,
    }

    if not accepted:
        args.output_ply.parent.mkdir(parents=True, exist_ok=True)
        args.output_ply.write_bytes(args.input_ply.read_bytes())
        report_data["status"] = "no_views_accepted; input PLY preserved unchanged"
        args.report.write_text(json.dumps(report_data, indent=2), encoding="utf-8")
        event("distill", 1.0, "No Harmonizer pseudo views passed safety gates; preserved input PLY")
        return

    device = torch.device("cuda")
    source = load_ply_to_splats(str(args.input_ply))
    means = torch.nn.Parameter(source["means"].to(device))
    scales = torch.nn.Parameter(source["scales"].to(device))
    opacities = torch.nn.Parameter(source["opacities"].to(device))
    sh0 = torch.nn.Parameter(source["sh0"].to(device))
    shn = torch.nn.Parameter(source["shN"].to(device))
    quats = source["quats"].to(device)

    anchor_means = means.detach().clone()
    anchor_scales = scales.detach().clone()
    anchor_opacities = opacities.detach().clone()
    scene_scale = anchor_means.std(dim=0).norm().clamp_min(1e-5)
    max_mean_delta = scene_scale * 0.005

    optimizer = torch.optim.Adam(
        [
            {"params": [means], "lr": 1e-5},
            {"params": [scales], "lr": 1e-4},
            {"params": [opacities], "lr": 3e-4},
            {"params": [sh0], "lr": 1e-3},
            {"params": [shn], "lr": 5e-5},
        ],
        eps=1e-8,
    )

    splats = {
        "means": means,
        "scales": scales,
        "quats": quats,
        "opacities": opacities,
        "sh0": sh0,
        "shN": shn,
    }

    distill_width = min(640, int(accepted[0]["width"]))
    distill_height = max(1, round(int(accepted[0]["height"]) * distill_width / int(accepted[0]["width"])))
    initial_mses: list[float] = []
    for item in accepted[:8]:
        raw = load_target(args.raw_dir / str(item["file"]), distill_width, distill_height, device)
        target = load_target(Path(str(item["fixed_path"])), distill_width, distill_height, device)
        initial_mses.append(float(F.mse_loss(raw, target).item()))

    for step in range(args.steps):
        item = accepted[step % len(accepted)]
        source_w = int(item["width"])
        source_h = int(item["height"])
        K = np.asarray(item["K"], dtype=np.float32).copy()
        K[0, :] *= distill_width / source_w
        K[1, :] *= distill_height / source_h
        c2w = np.asarray(item["c2w"], dtype=np.float32)
        target = load_target(Path(str(item["fixed_path"])), distill_width, distill_height, device)

        rendered = render_splats(splats, c2w, K, distill_width, distill_height)
        image_l1 = F.l1_loss(rendered, target)
        edges = edge_loss(rendered, target)
        geometry = ((means - anchor_means) / scene_scale).square().mean()
        scale_anchor = (scales - anchor_scales).square().mean()
        opacity_anchor = (opacities - anchor_opacities).square().mean()
        loss = image_l1 + 0.12 * edges + 0.02 * geometry + 0.002 * scale_anchor + 0.0005 * opacity_anchor

        optimizer.zero_grad(set_to_none=True)
        loss.backward()
        optimizer.step()

        with torch.no_grad():
            delta = means - anchor_means
            norms = torch.linalg.norm(delta, dim=-1, keepdim=True).clamp_min(1e-12)
            factor = torch.clamp(max_mean_delta / norms, max=1.0)
            means.copy_(anchor_means + delta * factor)
            scales.copy_(torch.maximum(torch.minimum(scales, anchor_scales + 0.15), anchor_scales - 0.15))
            opacities.copy_(torch.maximum(torch.minimum(opacities, anchor_opacities + 0.5), anchor_opacities - 0.5))

        if step % 25 == 0 or step + 1 == args.steps:
            event(
                "distill",
                (step + 1) / args.steps,
                f"Distilling {len(accepted)} accepted views; step {step + 1}/{args.steps}, loss={loss.item():.5f}",
            )

    final_mses: list[float] = []
    with torch.inference_mode():
        for item in accepted[:8]:
            source_w = int(item["width"])
            source_h = int(item["height"])
            K = np.asarray(item["K"], dtype=np.float32).copy()
            K[0, :] *= distill_width / source_w
            K[1, :] *= distill_height / source_h
            c2w = np.asarray(item["c2w"], dtype=np.float32)
            target = load_target(Path(str(item["fixed_path"])), distill_width, distill_height, device)
            rendered = render_splats(splats, c2w, K, distill_width, distill_height)
            final_mses.append(float(F.mse_loss(rendered, target).item()))

    args.output_ply.parent.mkdir(parents=True, exist_ok=True)
    export_splats(
        means.detach().cpu(),
        scales.detach().cpu(),
        quats.detach().cpu(),
        opacities.detach().cpu(),
        sh0.detach().cpu(),
        shn.detach().cpu(),
        format="ply",
        save_to=str(args.output_ply),
    )

    report_data.update(
        {
            "status": "distilled",
            "steps": args.steps,
            "gaussian_count": int(means.shape[0]),
            "validation_views": min(8, len(accepted)),
            "pseudo_target_psnr_before": psnr_from_mse(float(np.mean(initial_mses))),
            "pseudo_target_psnr_after": psnr_from_mse(float(np.mean(final_mses))),
            "accepted": [
                {
                    "file": item["file"],
                    "mean_change": item["mean_change"],
                    "edge_correlation": item["edge_correlation"],
                }
                for item in accepted
            ],
        }
    )
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report_data, indent=2), encoding="utf-8")
    event("distill", 1.0, f"Refined 3D representation written to {args.output_ply}")


def scale_command(args: argparse.Namespace) -> None:
    if args.factor <= 0 or not math.isfinite(args.factor):
        raise SystemExit("Scale factor must be a positive finite number")
    splats = load_ply_to_splats(str(args.input_ply))
    splats["means"] *= args.factor
    splats["scales"] += math.log(args.factor)
    args.output_ply.parent.mkdir(parents=True, exist_ok=True)
    export_splats(
        splats["means"],
        splats["scales"],
        splats["quats"],
        splats["opacities"],
        splats["sh0"],
        splats["shN"],
        format="ply",
        save_to=str(args.output_ply),
    )
    event("export", 1.0, f"Applied reconstruction scale x{args.factor:g}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)

    render = sub.add_parser("render")
    render.add_argument("--input-ply", type=Path, required=True)
    render.add_argument("--data-dir", type=Path, required=True)
    render.add_argument("--output-dir", type=Path, required=True)
    render.add_argument("--camera-json", type=Path, required=True)
    render.add_argument("--resolution", type=int, choices=sorted(RESOLUTION_MAP), default=1024)
    render.add_argument("--max-views", type=int, default=48)
    render.set_defaults(func=render_command)

    distill = sub.add_parser("distill")
    distill.add_argument("--input-ply", type=Path, required=True)
    distill.add_argument("--raw-dir", type=Path, required=True)
    distill.add_argument("--harmonized-dir", type=Path, required=True)
    distill.add_argument("--camera-json", type=Path, required=True)
    distill.add_argument("--output-ply", type=Path, required=True)
    distill.add_argument("--report", type=Path, required=True)
    distill.add_argument("--steps", type=int, default=750)
    distill.add_argument("--max-mean-change", type=float, default=0.22)
    distill.add_argument("--min-edge-correlation", type=float, default=0.35)
    distill.set_defaults(func=distill_command)

    scale = sub.add_parser("scale")
    scale.add_argument("--input-ply", type=Path, required=True)
    scale.add_argument("--output-ply", type=Path, required=True)
    scale.add_argument("--factor", type=float, required=True)
    scale.set_defaults(func=scale_command)
    return parser


def main() -> None:
    args = build_parser().parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
