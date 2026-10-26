<div align="center">

# ⚡ DeepSeek CLI (`deepseek`)

**The ultimate open-source, blazing-fast autonomous coding agent & terminal assistant for DeepSeek-V3 & DeepSeek-R1 reasoning models, written in pure Rust.**
*(The complete Claude Code & OpenCode experience, running on DeepSeek API and local LLMs)*

[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/surf1k/deepseek-cli?style=flat-square&logo=github)](https://github.com/surf1k/deepseek-cli/stargazers)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey.svg?style=flat-square)](https://github.com/surf1k/deepseek-cli)

<br/>

```
  ____                  ____            _       ____ _     ___ 
 |  _ \  ___  ___ _ __ / ___|  ___  ___| | __  / ___| |   |_ _|
 | | | |/ _ \/ _ \ '_ \\___ \ / _ \/ _ \ |/ / | |   | |    | | 
 | |_| |  __/  __/ |_) |___) |  __/  __/   <  | |___| |___ | | 
 |____/ \___|\___| .__/|____/ \___|\___|_|\_\  \____|_____|___|
                 |_|                                           
```

</div>

---

## ⚡ One-Line Install

Install instantly on Linux or macOS:
```bash
curl -fsSL https://raw.githubusercontent.com/surf1k/deepseek-cli/main/install.sh | bash
```

Or via Cargo:
```bash
cargo install --git https://github.com/surf1k/deepseek-cli
```

---

## ✨ Highlights

- 🤖 **Autonomous Coding Agent (`deepseek agent`)**: Replicates the **Claude Code / OpenCode** workflow. The agent autonomously plans, searches files (`glob`/`grep`), reads code (`read_file`), applies surgical edits (`edit_file`), and runs shell commands (`bash`) in an interactive loop.
- 👥 **Multi-Agent Delegation (`spawn_subagent`)**: The primary agent can spawn focused subagents (e.g. *Codebase Researcher*, *Test Runner*, *Security Auditor*) in separate sandboxed contexts to parallelize work without cluttering conversation memory.
- 🎨 **Visual Diff Engine**: Surgical file modifications previewed in color-coded unified diff format (`+` green / `-` red) before applying.
- 💰 **Real-Time Cost & Claude Savings Tracker (`/cost`)**: Calculates exact API cost down to the fraction of a cent based on DeepSeek's official pricing (including Context Cache hits at $0.14/1M) and shows exact dollar savings compared to Claude 3.5 Sonnet.
- 🗜️ **Context Compaction (`/compact` & Auto-Compaction)**: When conversations grow large, the compaction engine summarizes prior history while preserving architectural decisions and modified file states.
- 💾 **Session Persistence & Resume (`-C` / `--continue`)**: Pick up right where you left off across terminal sessions.
- 🛡️ **Fine-Grained Permission System**: Read-only actions run safely; mutating actions (`edit_file`, `write_file`, `bash`) require approval (`[y] Yes | [n] No | [a] Always allow`), or use `--auto` (`-y`) for unattended runs.
- 🧠 **Project Memory**: Automatically detects and ingests `DEEPSEEK.md`, `CLAUDE.md`, or `AGENTS.md` instructions from your repository.
- ⚡ **Blazing Fast (<10ms startup)**: Single static binary in pure async Rust (`tokio` + `reqwest`), no heavy Python or Node.js runtime required.
- 💭 **DeepThink (R1 Reasoning) Visualizer**: Live streaming of DeepSeek-R1's inner thought process with elapsed timer and reasoning token counter.

---

## ⚙️ Authentication & Configuration

When you launch `deepseek`, an interactive setup wizard guides you:
- 🌐 **Free Web Account (`chat.deepseek.com`) [100% FREE / NO API KEY]**: Connects directly to your regular free DeepSeek account with our built-in Keccak-f1600 Proof-of-Work (PoW) solver. Unlimited free queries with zero API cost!
- 🔑 **Developer API Key (`platform.deepseek.com`)**: For full autonomous tool execution (`edit_file`, `bash`) via official paid pay-as-you-go balance.
- ⚡ **Free OpenRouter (`deepseek/deepseek-r1:free`)**: Free API access via OpenRouter.
- 💻 **Custom Endpoints**: Connect to local Ollama (`http://localhost:11434/v1`), vLLM, or custom proxies.

### Login anytime:
```bash
deepseek login                    # Switch between Free Web Account, API Key, or OpenRouter
deepseek balance                  # Check your live account balance or free web status
```

Or set via environment variable:
```bash
export DEEPSEEK_API_KEY="sk-your-deepseek-api-key"
```

Or store directly in `~/.config/deepseek-cli/config.toml`:
```bash
deepseek config set-key sk-your-deepseek-api-key
```

### Useful Config Commands
```bash
deepseek config show              # Display current settings and auto-compact status
deepseek config set-model flash   # Set default model to DeepSeek-V4.1-Flash
deepseek config set-url <URL>     # Set custom base URL (e.g. http://localhost:11434/v1 for Ollama)
```

---

## 🤖 Autonomous Coding Agent (Claude Code in Pure Rust)

Let DeepSeek work directly inside your codebase:

```bash
# Interactive agent mode
deepseek agent "Fix the timeout panic in src/api.rs and run cargo test to verify"

# Auto-approved unattended mode
deepseek agent -y "Add healthcheck endpoint to server and update README.md"

# Short syntax
deepseek -a "Refactor database query logic in repository.rs"

# Resume previous session
deepseek -C -a "Now add unit tests for that refactor"
```

---

## 💡 Developer Workflows

### 1. Interactive Agent REPL
```bash
deepseek
```
* **Tab-autocompleted slash commands**:
  * `/model`, `/models` — Interactive model picker (or `/model v3`, `/model r1`).
  * `/autocompact` — Toggle automatic context compaction and view threshold.
  * `/compact` — Compress conversation history immediately.
  * `/cost` — Show exact session dollar cost and savings vs Claude 3.5.
  * `/status` — Show complete session status, context usage, and active model.
  * `/auto` — Toggle auto-approval on/off for tool executions.
  * `/sessions` — List saved conversation sessions.
  * `/copy` — Copy the last response to clipboard.
  * `/tokens` — Show session token counts and cache stats.
  * `/save [path]` — Export chat session to Markdown.
  * `/load <path>` — Inject file content into chat context.
  * `/retry` — Regenerate the last response.
  * `/exit` — Exit REPL.

### 2. Check Session Cost & Economics
```bash
deepseek cost
```
```text
Session Economics & Cost Breakdown:
  Model used:          deepseek-chat
  Cache hit tokens:    12,480 tokens ($0.14/1M)
  Cache miss tokens:   1,120 tokens ($0.28/1M)
  Completion tokens:   840 tokens ($2.19/1M)
  Total tokens:        14,440
  ---------------------------------------------
  DeepSeek API Cost:   $0.00390
  Claude 3.5 Cost:     $0.0534
  Money Saved:         $0.0495 🚀
```

### 3. AI Git Commit Generator
```bash
git add src/
deepseek commit
```

### 4. AI Code Review
```bash
deepseek review
# Or fast review with V3:
deepseek review -m v3
```

### 5. Natural Language Shell Helper
```bash
deepseek sh "find all files larger than 100MB and sort them by size"
```

### 6. Attach Files & Line Ranges
```bash
deepseek -f src/api.rs:40-95 "Review this stream handling logic"
deepseek -f "src/**/*.rs" "Suggest architectural improvements"
```

---

## 🛠️ CLI Reference

```
Usage: deepseek [OPTIONS] [PROMPT]... [COMMAND]

Commands:
  agent     Run an autonomous coding agent with file editing, search, and shell execution (alias: a)
  commit    Generate a Conventional Commit message from current git diff (alias: c)
  review    Perform a deep code review of staged/unstaged git changes (alias: r)
  sh        Generate and optionally execute a shell command from natural language (alias: run)
  sessions  Manage saved conversation sessions
  cost      Show token and dollar cost breakdown of the latest session
  config    Manage configuration, models, and API keys
  help      Print this message or help for given subcommand

Arguments:
  [PROMPT]...  Prompt / question (leave empty for REPL mode)

Options:
  -a, --agent                      Run in autonomous agent mode (like Claude Code)
  -y, --auto                       Auto-approve tool executions in agent mode
  -C, --continue                   Continue / resume previous conversation session
      --cost                       Show exact dollar cost breakdown of previous session
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
