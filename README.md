<div align="center">

# ⚡ DeepSeek CLI (`deepseek`)

**Blazing-fast, sleek terminal client and coding assistant for DeepSeek-V3 & DeepSeek-R1 reasoning models, written in pure Rust.**

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

- ⚡ **Instant & Lightweight**: Written in Rust with minimal dependencies, starts up in <10ms with zero runtime overhead.
- 🧠 **DeepThink / R1 Reasoning Visualization**: Native real-time streaming of DeepSeek-R1's inner thought process (`reasoning_content`) in a dedicated styled block before outputting the final answer.
- 💬 **Interactive REPL**: Rich terminal prompt with persistent command history, multiline editing (end with `\`), session saving, and in-chat model switching.
- 🔗 **Unix Pipelines**: Seamlessly pipe logs, code diffs, and terminal output:
  ```bash
  git diff | deepseek "Write a conventional commit message"
  ```
- 📂 **Multi-File Context**: Attach source code files directly to your query with `-f`:
  ```bash
  deepseek -f src/main.rs -f Cargo.toml "Review this code and suggest performance improvements"
  ```
- 🎯 **V3 & R1 Switcher**: Instantly switch between `deepseek-chat` (V3) and `deepseek-reasoner` (R1) via `-m v3` or `-m r1`.
- ⚙️ **Persistent Configuration**: Manage your API key and default preferences via `deepseek config`.
- 🛡️ **Raw Output Mode**: `--raw` flag outputs clean text without ANSI codes for scripts and automation.

---

## 🚀 Quick Start

### 1. Installation

#### Via Cargo
```bash
cargo install --git https://github.com/surf1k/deepseek-cli
```

#### Build from Source
```bash
git clone https://github.com/surf1k/deepseek-cli.git
cd deepseek-cli
cargo build --release
cp target/release/deepseek ~/.local/bin/ # or /usr/local/bin
```

---

### 2. Configure API Key

Set via environment variable:
```bash
export DEEPSEEK_API_KEY="sk-your-deepseek-api-key"
```
Or save it persistently to `~/.config/deepseek-cli/config.toml`:
```bash
deepseek config set-key sk-your-deepseek-api-key
```

Verify your config anytime:
```bash
deepseek config show
```

---

## 💡 Usage Examples

### 1. Interactive REPL Mode
Simply run `deepseek` without arguments:
```bash
deepseek
```
Inside the REPL:
- Type your prompt and press Enter.
- End a line with `\` to write multiline prompts.
- `/model r1` — Switch to DeepSeek-R1 (DeepThink reasoning).
- `/model v3` — Switch to DeepSeek-V3 (Fast chat).
- `/clear` — Clear conversation context.
- `/save session.md` — Export current conversation to Markdown.
- `/system <prompt>` — Update system prompt on the fly.
- `/exit` — Quit.

### 2. One-Shot Question
```bash
deepseek "Explain the difference between Arc and Rc in Rust in 3 bullet points"
```

### 3. Reasoning with DeepSeek-R1
```bash
deepseek -m r1 "Solve this riddle: A farmer has 17 sheep, all but 9 die. How many are left?"
```

### 4. Piped Input / Code Review
```bash
cat error.log | deepseek "Analyze this stacktrace and explain why it crashed"
```
```bash
git diff | deepseek -m v3 "Draft a concise release note for this diff"
```

### 5. Attach Local Files
```bash
deepseek -f Cargo.toml -f src/config.rs "Does this config follow Rust best practices?"
```

---

## 🛠️ CLI Options

```
Usage: deepseek [OPTIONS] [PROMPT]... [COMMAND]

Commands:
  config  Manage configuration and API keys
  help    Print this message or the help of the given subcommand(s)

Arguments:
  [PROMPT]...  Prompt / question to ask DeepSeek (leave empty for REPL mode)

Options:
  -m, --model <MODEL>              Model: 'v3' (deepseek-chat) or 'r1' (deepseek-reasoner)
  -s, --system <SYSTEM>            Custom system prompt
  -f, --file <FILE>                Attach file(s) into context
  -t, --temperature <TEMPERATURE>  Sampling temperature (0.0 to 2.0)
      --raw                        Plain-text output (no styling, ideal for scripts)
      --base-url <BASE_URL>        Custom base URL (default: https://api.deepseek.com)
  -h, --help                       Print help
  -V, --version                    Print version
```

---

## 🤝 Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## 📄 License

Distributed under the MIT License. See [LICENSE](LICENSE) for more information.
