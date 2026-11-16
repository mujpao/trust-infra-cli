# TrustInfra experiment setup: the "recipe" from paper §3.1 step 1.
# Two containers on one private network:
#   ollama : serves the model (image pinned by digest)
#   eval   : runs Inspect against ollama (image pinned by digest)
# Everything a reviewer should be able to check is a variable, and the values
# live in declared.auto.tfvars (committed to git = the declared setup).

terraform {
  required_providers {
    docker = {
      source  = "kreuzwerker/docker"
      version = "~> 3.0"
    }
  }
}

provider "docker" {} # uses /var/run/docker.sock (Docker Desktop)

# ---- declared setup (values in declared.auto.tfvars) -----------------------
variable "ollama_image" { type = string }       # repo@sha256:...
variable "eval_image" { type = string }         # repo@sha256:...
variable "model_tag" { type = string }          # Ollama tag the eval asks for
variable "model_digest" { type = string }       # expected Ollama model digest
variable "model_quantization" { type = string } # expected quantization level
variable "inspect_ai_version" { type = string }
variable "inspect_evals_version" { type = string }
variable "task" { type = string }
variable "task_args" { type = map(string) }
variable "limit" { type = number }
variable "temperature" { type = number }
variable "seed" { type = number }
variable "max_tokens" { type = number }

# ---- per-run, NOT part of the declared setup -------------------------------
variable "run_dir" {
  type        = string
  description = "Absolute host path where this run writes its outputs"
}

resource "docker_network" "net" {
  name = "trustinfra"
}

# Kept across runs so the model and dataset are downloaded once.
resource "docker_volume" "models" { name = "trustinfra-ollama-models" }
resource "docker_volume" "hf_cache" { name = "trustinfra-hf-cache" }
resource "docker_volume" "pip_cache" { name = "trustinfra-pip-cache" }

resource "docker_image" "ollama" {
  name         = var.ollama_image
  keep_locally = true
}

resource "docker_image" "eval" {
  name         = var.eval_image
  keep_locally = true
}

resource "docker_container" "ollama" {
  name  = "trustinfra-ollama"
  image = docker_image.ollama.image_id

  # Explicit defaults: the provider reads these back as []/{} and would
  # otherwise report "changed outside of OpenTofu" on every clean run.
  dns          = []
  dns_opts     = []
  dns_search   = []
  group_add    = []
  log_opts     = {}
  storage_opts = {}
  sysctls      = {}
  tmpfs        = {}
  networks_advanced {
    name    = docker_network.net.id
    aliases = ["ollama"]
  }
  volumes {
    volume_name    = docker_volume.models.name
    container_path = "/root/.ollama"
    read_only      = false
  }
}

resource "docker_container" "eval" {
  name    = "trustinfra-eval"
  image   = docker_image.eval.image_id
  command = ["sleep", "infinity"]

  # Explicit defaults: the provider reads these back as []/{} and would
  # otherwise report "changed outside of OpenTofu" on every clean run.
  dns          = []
  dns_opts     = []
  dns_search   = []
  group_add    = []
  log_opts     = {}
  storage_opts = {}
  sysctls      = {}
  tmpfs        = {}
  env = [
    "OLLAMA_BASE_URL=http://ollama:11434/v1",
    "HF_HOME=/hf",
  ]
  networks_advanced {
    name = docker_network.net.id
  }
  volumes {
    host_path      = abspath("${path.module}/../experiments/eval")
    container_path = "/exp"
    read_only      = true
  }
  volumes {
    host_path      = var.run_dir
    container_path = "/work"
    read_only      = false
  }
  volumes {
    volume_name    = docker_volume.hf_cache.name
    container_path = "/hf"
    read_only      = false
  }
  volumes {
    volume_name    = docker_volume.pip_cache.name
    container_path = "/root/.cache/pip"
    read_only      = false
  }
}

# The declared setup as one object. run_eval.sh reads it with `tofu console`
# BEFORE apply and without any -var override, so it always reflects the committed
# tfvars file, even when a drift run overrides what is actually applied.
output "declared" { value = local.declared }

locals {
  declared = {
    ollama_image          = var.ollama_image
    eval_image            = var.eval_image
    model_tag             = var.model_tag
    model_digest          = var.model_digest
    model_quantization    = var.model_quantization
    inspect_ai_version    = var.inspect_ai_version
    inspect_evals_version = var.inspect_evals_version
    task                  = var.task
    task_args             = var.task_args
    limit                 = var.limit
    temperature           = var.temperature
    seed                  = var.seed
    max_tokens            = var.max_tokens
  }
}

output "ollama_image_id" { value = docker_image.ollama.image_id }
output "eval_image_id" { value = docker_image.eval.image_id }
output "ollama_container_id" { value = docker_container.ollama.id }
output "eval_container_id" { value = docker_container.eval.id }
