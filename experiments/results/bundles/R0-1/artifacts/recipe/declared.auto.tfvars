# The DECLARED setup. Committed to git before the runs; the manifest's
# infra_declared is built from this file. Drift = what ran differs from this.

ollama_image = "ollama/ollama@sha256:910d0d8107b538fb0894e63163eb195de0cc7aefcbc666dc62d3b18f71124592" # ollama 0.40.16
eval_image   = "python@sha256:e48f59cdba69a3cbd9ed2cf0f348d90d45a8d350d98134cf1070bc106ee9ed90"        # python:3.13-slim

model_tag = "qwen2.5:0.5b"
# Ollama manifest digest (covers weights, template, system prompt, params).
# Weights layer alone: sha256:c5396e06af294bd101b30dce59131a76d2b773e76950acc870eda801d3ab0515
model_digest       = "sha256:a8b0c51577010a279d933d14c2a8ab4b268079d44c5c8830c0a93900f1827c67"
model_quantization = "Q4_K_M"

inspect_ai_version    = "0.3.290"
inspect_evals_version = "0.29.0"

task = "inspect_evals/truthfulqa"
task_args = {
  target          = "mc1"
  shuffle         = "false" # first 100 questions in source order
  shuffle_choices = "42"    # correct answer is always A in the source; keep fixed shuffle
}
limit       = 100
temperature = 0
seed        = 42
max_tokens  = 256 # cap per answer; the reply format is 'ANSWER: X'. Without it a looping answer ran past 6,800 tokens
