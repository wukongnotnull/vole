# Phase 4c Batch 3 selection

Target: **40** rules  
**Actual: TBD** (planning doc only; freeze during implementation Task 2)

Baseline after Batch 2 merge: **46** enabled rules (40 inventory-ported + AI/Codex/example); **416** unported `all` candidates.

## Block A — `data/rules/app-caches.toml` (`all`, +18)

| proposed_id | label | path | strategy |
|---|---|---|---|
| whatsapp-cache | WhatsApp cache | `~/Library/Caches/net.whatsapp.WhatsApp/*` | all |
| skype-cache | Skype cache | `~/Library/Caches/com.skype.skype/*` | all |
| tencent-meeting-cache | Tencent Meeting cache | `~/Library/Caches/com.tencent.meeting/*` | all |
| wecom-cache | WeCom cache | `~/Library/Caches/com.tencent.WeWorkMac/*` | all |
| qq-cache | QQ cache | `~/Library/Caches/com.tencent.qq/*` | all |
| feishu-cache | Feishu cache | `~/Library/Caches/com.feishu.*/*` | all |
| teams-legacy-cache | Microsoft Teams legacy cache | `~/Library/Application Support/Microsoft/Teams/Cache/*` | all |
| teams-legacy-logs | Microsoft Teams legacy logs | `~/Library/Application Support/Microsoft/Teams/logs/*` | all |
| teams-legacy-tmp | Microsoft Teams legacy temp files | `~/Library/Application Support/Microsoft/Teams/tmp/*` | all |
| dingtalk-cache | DingTalk iDingTalk cache | `~/Library/Caches/dd.work.exclusive4aliding/*` | all |
| dingtalk-logs | DingTalk logs | `~/Library/Application Support/iDingTalk/log/*` | all |
| chatgpt-cache | ChatGPT cache | `~/Library/Caches/com.openai.chat/*` | all |
| claude-desktop-cache | Claude desktop cache | `~/Library/Caches/com.anthropic.claudefordesktop/*` | all |
| claude-logs | Claude logs | `~/Library/Logs/Claude/*` | all |
| lm-studio-cache | LM Studio cache | `~/Library/Caches/com.lmstudio.lmstudio/*` | all |
| sketch-cache | Sketch cache | `~/Library/Caches/com.bohemiancoding.sketch3/*` | all |
| adobe-cache | Adobe cache | `~/Library/Caches/Adobe/*` | all |
| screenflow-cache | ScreenFlow cache | `~/Library/Caches/net.telestream.screenflow10/*` | all |

## Block B — `data/rules/user-devtools.toml` (`all`, +22)

| proposed_id | label | path | strategy |
|---|---|---|---|
| tnpm-cacache | tnpm cache directory | `~/.tnpm/_cacache/*` | all |
| yarn-cache | Yarn cache | `~/.yarn/cache/*` | all |
| yarn-v1-cache | Yarn v1 cache | `~/Library/Caches/Yarn/*` | all |
| pyenv-cache | pyenv cache | `~/.pyenv/cache/*` | all |
| poetry-cache | Poetry cache | `~/.cache/poetry/*` | all |
| ruff-cache | Ruff cache | `~/.cache/ruff/*` | all |
| mypy-cache | MyPy cache | `~/.cache/mypy/*` | all |
| pytest-cache | Pytest cache | `~/.pytest_cache/*` | all |
| jupyter-runtime | Jupyter runtime cache | `~/.jupyter/runtime/*` | all |
| huggingface-cache | Hugging Face cache | `~/.cache/huggingface/*` | all |
| pytorch-cache | PyTorch cache | `~/.cache/torch/*` | all |
| tensorflow-cache | TensorFlow cache | `~/.cache/tensorflow/*` | all |
| wandb-cache | Weights & Biases cache | `~/.cache/wandb/*` | all |
| cargo-registry-cache | Rust cargo cache | `~/.cargo/registry/cache/*` | all |
| cargo-git-cache | Cargo git cache | `~/.cargo/git/*` | all |
| rustup-downloads | Rust downloads cache | `~/.rustup/downloads/*` | all |
| rbenv-cache | rbenv download cache | `~/.rbenv/cache/*` | all |
| gem-spec-cache | gem spec cache | `~/.gem/specs/*` | all |
| bundler-cache | Ruby Bundler cache | `~/.bundle/cache/*` | all |
| docker-buildx-cache | Docker BuildX cache | `~/.docker/buildx/cache/*` | all |
| kube-cache | Kubernetes cache | `~/.kube/cache/*` | all |
| cpan-build | CPAN build artifacts | `~/.cpan/build/*` | all |

## Excluded this batch

- `user.sh` broad sweeps (`~/Library/Caches/*`, `~/Library/Logs/*`, …)
- `not_running` / `pgrep` guard rules
- symlink / custom / sudo rules
- new `custom` handlers (quota: 0)

## Custom ratio target

Pre-batch: 3 custom / 46 rules ≈ 6.5%.  
Post-batch (if +40 all): 3 / 86 ≈ 3.5% (≤ 5% gate).
