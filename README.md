<div align="center">

# ⚡ DeepSeek CLI (`deepseek`)

**The ultimate terminal client, coding assistant, and Git workflow tool for DeepSeek-V3 & DeepSeek-R1 reasoning models, written in pure Rust.**

[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/surf1k/deepseek-cli?style=flat-square&logo=github)](https://github.com/surf1k/deepseek-cli/stargazers)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey.svg?style=flat-square)](https://github.com/surf1k/deepseek-cli)

<br/>

```
   ___               ____            _      ____ _     ___ 
  / _ \___ ___ ___  / __/__ ___ k   | |    / ___| |   |_ _|
 / // / -_) -_) _ \_\ \/ -_) -_) k  | |   | |   | |    | | 
/____/\__/\__/_//_/___/\__/\__/_/_/ |_|   |_|   |___| |___|
```

</div>

---

## ✨ Features

- ⚡ **Blazing Fast (<10ms startup)**: Single static binary in pure async Rust (`tokio` + `reqwest` + `rustyline`), instant startup with zero runtime overhead.
- 🧠 **DeepThink / R1 Reasoning Visualization**: Real-time streaming of DeepSeek-R1's inner thought process in a dedicated styled container, with elapsed timer and reasoning token counter.
- 📊 **Real-time Performance & Cache Stats**: Displays generation speed (`tok/s`), total tokens, elapsed time, and DeepSeek **Context Cache hit percentage** after each completion.
- 🛠️ **AI Git Commit Assistant (`deepseek commit`)**: Analyzes staged/unstaged git diffs, produces clean Conventional Commit messages, and allows one-key commit, edit, or regenerate.
- 🔍 **AI Code Reviewer (`deepseek review`)**: Thoroughly inspects current git diffs for bugs, memory issues, security vulnerabilities, and code quality.
- 🐚 **Natural Language Shell Assistant (`deepseek sh`)**: Converts natural language into executable shell commands for your OS/shell with a 1-click execution prompt.
- 💬 **Interactive REPL with Tab Autocompletion**: Multi-turn conversation with slash command autocomplete (`/help`, `/model`, `/copy`, `/tokens`, `/save`, `/load`, `/retry`), multiline inputs (end with `\`), and persistent history.
- 📋 **Seamless Clipboard Integration**: Auto-copy with `--copy` or `/copy` in REPL (native OS clipboard + OSC 52 fallback for SSH/tmux).
- 📂 **Flexible File Attachments**: Attach files (`-f src/main.rs`), specific line ranges (`-f src/main.rs:10-50`), or glob patterns (`-f "src/**/*.rs"`).
- 🔗 **Unix Pipelines**: Effortlessly pipe logs, diffs, and data (`git diff | deepseek "Explain this"`).
- 🌐 **Provider Agnostic**: Works out of the box with official DeepSeek API, or local LLMs via Ollama, vLLM, and OpenRouter using `--base-url`.

---

## 🚀 Installation

### Via Cargo
```bash
cargo install --git https://github.com/surf1k/deepseek-cli
```

### Build from Source
```bash
git clone https://github.com/surf1k/deepseek-cli.git
cd deepseek-cli
cargo build --release
cp target/release/deepseek ~/.local/bin/ # or /usr/local/bin
```

---

## ⚙️ Configuration

Set your API key via environment variable:
```bash
export DEEPSEEK_API_KEY="sk-your-deepseek-api-key"
```

Or store it persistently in `~/.config/deepseek-cli/config.toml`:
```bash
deepseek config set-key sk-your-deepseek-api-key
```

### Useful Config Commands
```bash
deepseek config show              # Display current settings
deepseek config set-model r1      # Set default model to DeepSeek-R1
deepseek config set-url <URL>     # Set custom base URL (e.g. for Ollama)
deepseek config path              # Print path to config.toml
```

---

## 💡 Workflows & Examples

### 1. Interactive REPL Mode
```bash
deepseek
```
* **Tab-autocompleted slash commands**:
  * `/copy` — Copy the last response to clipboard.
  * `/tokens` — Show session token counts and cache stats.
  * `/model <v3|r1>` — Hot-switch active model mid-conversation.
  * `/save [path]` — Export chat session to Markdown.
  * `/load <path>` — Inject file content into chat context.
  * `/retry` — Regenerate the last response.
  * `/temp <0.0-2.0>` — Adjust sampling temperature on the fly.
  * `/clear` — Clear context while keeping system prompt.
  * `/exit` — Exit REPL.

---

### 2. AI Git Commit Generator
Stage your files and let DeepSeek draft a conventional commit message:
```bash
git add src/
deepseek commit
```
*Interactive Prompt*:
```text
Proposed Commit Message:
──────────────────────────────────────────────
feat(api): add context caching stats & token usage streaming
──────────────────────────────────────────────
Action? [c] Commit | [e] Edit | [r] Regenerate | [q] Cancel:
```

---

### 3. AI Code Review
Run a comprehensive code audit on your active changes before pushing:
```bash
deepseek review
# Or with DeepSeek-V3 for fast review:
deepseek review -m v3
```

---

### 4. Natural Language Shell Helper
Need a complex bash one-liner? Just describe what you want:
```bash
deepseek sh "find all files larger than 100MB and sort them by size"
```
```text
╭─── Suggested Command ──────────────────────────────────
│  find . -type f -size +100M -exec ls -lh {} + | sort -k 5 -hr
╰── Searches for files >100MB and lists them in human-readable order.

Action? [y] Execute | [c] Copy to clipboard | [n] Cancel: 
```

---

### 5. File & Line-Range Context
```bash
# Attach specific line numbers
deepseek -f src/api.rs:40-95 "Review this stream handling logic"

# Attach all Rust files using glob
deepseek -f "src/**/*.rs" "Suggest architectural improvements"
```

---

### 6. Unix Pipelines
```bash
cat /var/log/nginx/error.log | deepseek "Analyze root cause of these 502 errors"
git diff main | deepseek "Generate a Markdown release summary"
```

---

### 7. Reasoning with DeepSeek-R1 (DeepThink)
```bash
deepseek -m r1 "How many r's are in strawberry? Prove step by step."
```

---

## 🛠️ CLI Reference

```
Usage: deepseek [OPTIONS] [PROMPT]... [COMMAND]

Commands:
  commit  Generate a Conventional Commit message from current git diff (alias: c)
  review  Perform a deep code review of staged/unstaged git changes (alias: r)
  sh      Generate and optionally execute a shell command from natural language (alias: run)
  config  Manage configuration, models, and API keys
  help    Print this message or help for given subcommand

Arguments:
  [PROMPT]...  Prompt / question (leave empty for REPL mode)

Options:
  -m, --model <MODEL>              Model: 'v3' (deepseek-chat) or 'r1' (deepseek-reasoner)
  -s, --system <SYSTEM>            Custom system prompt
  -f, --file <SPEC>                Attach file, range, or glob (-f src/main.rs:10-50 -f "src/**/*.rs")
  -t, --temperature <TEMPERATURE>  Sampling temperature (0.0 to 2.0)
      --max-tokens <MAX_TOKENS>    Maximum tokens to generate
      --raw                        Raw plain-text output (for scripting and pipes)
      --no-stats                   Hide the token & speed statistics bar
  -c, --copy                       Automatically copy final answer to system clipboard
      --base-url <BASE_URL>        Custom API base URL (default: https://api.deepseek.com)
  -h, --help                       Print help
  -V, --version                    Print version
```

---

## 🤝 Contributing

Contributions, feature requests, and bug reports are warmly welcome!

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feat/super-feature`)
3. Commit your Changes (`git commit -m 'feat: add super feature'`)
4. Push to the Branch (`git push origin feat/super-feature`)
5. Open a Pull Request

---

## 📄 License

Distributed under the MIT License. See [LICENSE](LICENSE) for more details.
