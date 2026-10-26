<div align="center">

# ⚡ DeepSeek CLI (`deepseek`)

**The open-source, blazing-fast autonomous coding agent & terminal assistant for DeepSeek-V3 & DeepSeek-R1 reasoning models, written in pure Rust.**
*(Full Claude Code & OpenCode experience powered by DeepSeek API & local LLMs)*

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

## ✨ Highlights

- 🤖 **Autonomous Coding Agent (`deepseek agent`)**: Replicates the **Claude Code / OpenCode** workflow. The agent autonomously plans, searches files (`glob`/`grep`), reads code (`read_file`), applies surgical edits (`edit_file`), and runs shell commands (`bash`) in an interactive loop until your task is done.
- 🎨 **Visual Diff Engine**: Surgical file modifications previewed in color-coded unified diff format (`+` green / `-` red) before applying.
- 🛡️ **Fine-Grained Permission System**: Read-only actions run safely; mutating actions (`edit_file`, `write_file`, `bash`) require approval (`[y] Yes | [n] No | [a] Always allow`), or use `--auto` (`-y`) for unattended background tasks.
- 🧠 **Project Memory**: Automatically detects and ingests `DEEPSEEK.md`, `CLAUDE.md`, or `AGENTS.md` instructions from your workspace.
- ⚡ **Blazing Fast (<10ms startup)**: Single static binary in pure async Rust (`tokio` + `reqwest`), no heavy Python or Node.js runtime required.
- 💭 **DeepThink (R1 Reasoning) Visualizer**: Live streaming of DeepSeek-R1's inner thought process with elapsed timer and reasoning token counter.
- 📊 **Real-time Performance & Cache Stats**: Displays tokens/second speed, total token counts, and DeepSeek **Context Cache hit percentage**.
- 🛠️ **Built-in Developer Utilities**:
  - `deepseek commit`: Automated Conventional Commits from staged/unstaged diffs.
  - `deepseek review`: Security & quality code review of active changes.
  - `deepseek sh`: Natural language terminal assistant with one-click command execution.
- 💬 **Interactive REPL**: Multi-turn dialogue with Tab autocompletion (`/copy`, `/tokens`, `/model`, `/save`, `/load`, `/retry`), multiline input (`\`), and persistent history.

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
deepseek config set-url <URL>     # Set custom base URL (e.g. http://localhost:11434/v1 for Ollama)
```

---

## 🤖 Autonomous Agent Mode (Claude Code in Rust)

Let DeepSeek work directly inside your codebase:

```bash
# Interactive agent mode
deepseek agent "Fix the timeout panic in src/api.rs and run cargo test to verify"

# Auto-approved unattended mode
deepseek agent -y "Add healthcheck endpoint to server and update README.md"

# Short syntax
deepseek -a "Refactor database query logic in repository.rs"
```

### How the Agent Works:
1. **Discovers**: Uses `grep` and `glob` to locate relevant files.
2. **Inspects**: Uses `read_file` to view lines of code without blowing context.
3. **Edits**: Uses `edit_file` to perform exact character replacements and shows a color diff.
4. **Verifies**: Runs `bash` (e.g. `cargo test`, `npm test`) to verify the fix works.
5. **Wraps up**: Explains what was changed.

---

## 💡 Developer Workflows

### 1. Interactive REPL Chat
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
  * `/exit` — Exit REPL.

### 2. AI Git Commit Generator
Stage your changes and generate a clean conventional commit:
```bash
git add src/
deepseek commit
```

### 3. AI Code Review
```bash
deepseek review
# Or fast review with V3:
deepseek review -m v3
```

### 4. Natural Language Shell Helper
```bash
deepseek sh "find all files larger than 100MB and sort them by size"
```

### 5. Attach Files & Line Ranges
```bash
deepseek -f src/api.rs:40-95 "Review this stream handling logic"
deepseek -f "src/**/*.rs" "Suggest architectural improvements"
```

---

## 🛠️ CLI Reference

```
Usage: deepseek [OPTIONS] [PROMPT]... [COMMAND]

Commands:
  agent   Run an autonomous coding agent with file editing, search, and shell execution (alias: a)
  commit  Generate a Conventional Commit message from current git diff (alias: c)
  review  Perform a deep code review of staged/unstaged git changes (alias: r)
  sh      Generate and optionally execute a shell command from natural language (alias: run)
  config  Manage configuration, models, and API keys
  help    Print this message or help for given subcommand

Arguments:
  [PROMPT]...  Prompt / question (leave empty for REPL mode)

Options:
  -a, --agent                      Run in autonomous agent mode (like Claude Code)
  -y, --auto                       Auto-approve tool executions in agent mode
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
