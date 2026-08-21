#!/usr/bin/env bash
set -Eeuo pipefail

if [[ "${EUID}" -ne 0 ]]; then
  echo "This installer must run as root inside the dedicated WSL distro." >&2
  exit 1
fi

GSS_ROOT="${GSS_ROOT:-/opt/gss}"
VENV="$GSS_ROOT/venv"
UPSTREAM="$GSS_ROOT/upstream"
STAMP="$GSS_ROOT/.stamps"
CUDA_HOME="/usr/local/cuda-12.8"
export DEBIAN_FRONTEND=noninteractive
export CUDA_HOME
export PATH="$CUDA_HOME/bin:$PATH"

mkdir -p "$GSS_ROOT" "$UPSTREAM" "$STAMP" "$GSS_ROOT/cache" "$GSS_ROOT/models"

event() {
  local stage="$1"
  local progress="$2"
  shift 2
  python3 - "$stage" "$progress" "$*" <<'PY'
import json, sys
print("GSS_EVENT " + json.dumps({"stage": sys.argv[1], "progress": float(sys.argv[2]), "message": sys.argv[3]}), flush=True)
PY
}

step() {
  local name="$1"
  shift
  if [[ -f "$STAMP/$name" ]]; then
    echo "[GSS] $name already completed"
    return 0
  fi
  "$@"
  touch "$STAMP/$name"
}

install_base() {
  apt-get update
  apt-get install -y --no-install-recommends \
    ca-certificates curl wget gnupg2 git git-lfs build-essential cmake ninja-build pkg-config \
    python3 python3-venv python3-dev python3-pip ffmpeg colmap docker.io jq \
    libgl1 libglib2.0-0 libx11-6 libxext6 libsm6 libgomp1
  git lfs install --system
}

install_cuda_toolkit() {
  if [[ -x "$CUDA_HOME/bin/nvcc" ]]; then
    return 0
  fi
  local keyring=/tmp/cuda-keyring_1.1-1_all.deb
  wget -q https://developer.download.nvidia.com/compute/cuda/repos/wsl-ubuntu/x86_64/cuda-keyring_1.1-1_all.deb -O "$keyring"
  dpkg -i "$keyring"
  apt-get update
  apt-get install -y cuda-toolkit-12-8
  cat >/etc/profile.d/gss-cuda.sh <<'EOF'
export CUDA_HOME=/usr/local/cuda-12.8
export PATH=/usr/local/cuda-12.8/bin:$PATH
EOF
}

install_container_toolkit() {
  curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey \
    | gpg --dearmor --yes -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
  curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list \
    | sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' \
    > /etc/apt/sources.list.d/nvidia-container-toolkit.list
  apt-get update
  apt-get install -y nvidia-container-toolkit
  nvidia-ctk runtime configure --runtime=docker

  if command -v systemctl >/dev/null 2>&1 && systemctl is-system-running >/dev/null 2>&1; then
    systemctl enable --now docker
    systemctl restart docker
  else
    service docker start || true
  fi
}

install_python() {
  python3 -m venv "$VENV"
  "$VENV/bin/python" -m pip install --upgrade pip setuptools wheel
  "$VENV/bin/pip" install --index-url https://download.pytorch.org/whl/cu128 torch torchvision
  "$VENV/bin/pip" install \
    numpy scipy pillow imageio imageio-ffmpeg opencv-python-headless tqdm natsort \
    "huggingface_hub[cli]" pycolmap ninja
}

install_gsplat() {
  if [[ ! -d "$UPSTREAM/gsplat/.git" ]]; then
    git clone --depth 1 https://github.com/nerfstudio-project/gsplat.git "$UPSTREAM/gsplat"
  else
    git -C "$UPSTREAM/gsplat" pull --ff-only
  fi

  CUDA_HOME="$CUDA_HOME" "$VENV/bin/pip" install -e "$UPSTREAM/gsplat"
  if [[ -f "$UPSTREAM/gsplat/examples/requirements.txt" ]]; then
    "$VENV/bin/pip" install -r "$UPSTREAM/gsplat/examples/requirements.txt"
  fi
}

install_spz() {
  if [[ ! -d "$UPSTREAM/spz/.git" ]]; then
    git clone --depth 1 https://github.com/nianticlabs/spz.git "$UPSTREAM/spz"
  else
    git -C "$UPSTREAM/spz" pull --ff-only
  fi
  "$VENV/bin/pip" install "$UPSTREAM/spz"
}

install_harmonizer() {
  if [[ ! -d "$UPSTREAM/harmonizer/.git" ]]; then
    git clone --depth 1 https://github.com/NVIDIA/harmonizer.git "$UPSTREAM/harmonizer"
  else
    git -C "$UPSTREAM/harmonizer" pull --ff-only
  fi

  if ! docker image inspect gss-harmonizer:latest >/dev/null 2>&1; then
    docker build -t gss-harmonizer:latest -f "$UPSTREAM/harmonizer/Dockerfile.cosmos" "$UPSTREAM/harmonizer"
  fi
}

verify_runtime() {
  nvidia-smi
  "$VENV/bin/python" - <<'PY'
import torch
import gsplat
assert torch.cuda.is_available(), "PyTorch cannot see the WSL NVIDIA GPU"
print("PyTorch:", torch.__version__)
print("CUDA:", torch.version.cuda)
print("GPU:", torch.cuda.get_device_name(0))
print("gsplat:", getattr(gsplat, "__version__", "installed"))
PY
  docker info >/dev/null
  nvidia-ctk --version
  colmap -h >/dev/null
  ffmpeg -version >/dev/null
}

event setup 0.02 "Installing Linux base packages"
step base install_base

event setup 0.18 "Installing CUDA 12.8 toolkit for gsplat compilation; no Linux display driver is installed"
step cuda install_cuda_toolkit

event setup 0.32 "Installing Docker NVIDIA Container Toolkit"
step container_toolkit install_container_toolkit

event setup 0.46 "Creating Python CUDA environment"
step python install_python

event setup 0.60 "Installing current gsplat/3DGUT backend"
step gsplat install_gsplat

event setup 0.72 "Installing official SPZ Python bindings"
step spz install_spz

event setup 0.80 "Building NVIDIA Harmonizer container"
step harmonizer install_harmonizer

event setup 0.94 "Verifying CUDA, gsplat, Docker, COLMAP and FFmpeg"
verify_runtime

event setup 1.0 "Runtime ready. Authenticate Hugging Face and download the selected model pack next."
