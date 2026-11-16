# Wall Street Champion

Turn an inherited **$100,000** into **$1,000,000** in **26 weeks** — or end up
pennyless. All stocks, ETFs, crypto coins, people and news are fictional.

Rust + Vulkan + FSR 1.x (FidelityFX Super Resolution EASU/RCAS compute passes).
Runs on Linux, FreeBSD and MidnightBSD. See [plan.md](plan.md) for the full
design.

## Quick start

```sh
cargo build --release

# Play in a window (needs a Vulkan GPU + X11/Wayland)
./target/release/wall-street-champion

# Play in the terminal (servers, ssh, CI)
./target/release/wall-street-champion --headless

# Auto-played demo / smoke test
./target/release/wall-street-champion --headless --bot

# Render frames headless on any Vulkan device (real GPU or lavapipe) and dump a PNG
./target/release/wall-street-champion --gpu-headless --frames 30 --screenshot shot.png
```

## Controls (window mode)

| Key | Action |
| --- | --- |
| `1..9,0,-,=` | select instrument |
| `B` / `Shift+B` | buy $1,000 / buy max of selected |
| `S` / `Shift+S` | sell 10 shares / sell all of selected |
| `E` | execute queued trades at close & advance one week |
| `T` then `1..4` | follow a tipster's tip |
| `N` / `V` | cycle newspaper / TV pages |
| `F` | toggle FSR 1.x upscaling |
| `L` | toggle local-LLM news (Ollama) |
| `H` | help, `Q` | quit |

Terminal mode uses the same commands as text.

## Local AI news (optional)

If [Ollama](https://ollama.com) is running locally (`ollama serve`), headline
bodies are written by a local model (`WSC_MODEL`, default `llama3.1:latest`),
using the GPU when available. The game never calls any cloud API and silently
falls back to procedural headlines when no server answers.

```sh
ollama serve &            # if not already running
./target/release/wall-street-champion --llm
```

## Tests

```sh
cargo test
```

## License

GPLv2 — see [LICENSE](LICENSE). Notes: the 8x8 bitmap font in `assets/` is
public domain (font8x8 / IBM VGA font), and the FSR 1.x shader passes are
ports of AMD's MIT-licensed FidelityFX-FSR reference implementation.
