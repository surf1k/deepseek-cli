# DeepSeek Code Guidelines (`DEEPSEEK.md`)

This document provides system instructions, architectural context, and behavioral guidelines for AI coding assistants (DeepSeek, Claude, and autonomous agents) operating within the `deepseek-cli` repository.

---

## 1. Project Overview & Architecture

**`deepseek-cli`** is a high-performance, single-binary autonomous coding agent, terminal assistant, and REPL for DeepSeek-V3 and DeepSeek-R1 reasoning models, built in 100% pure Rust.

### Directory Structure

```
deepseek-cli/
├── Cargo.toml               # Package dependencies & metadata
├── LICENSE                  # MIT License
├── README.md                # User-facing documentation & showcase
├── DEEPSEEK.md              # AI system instructions (this file)
├── src/
│   ├── main.rs              # CLI entry point, clap argument parsing, subcommand routing
│   ├── api.rs               # DeepSeek & OpenAI-compatible client, SSE streaming, tool-calling
│   ├── agent/               # Autonomous ReAct agent loop
│   │   └── mod.rs           # Multi-turn agent, permission engine, subagent delegation
│   ├── tools/               # Agent tool implementations
│   │   ├── mod.rs           # Tool dispatcher & JSON Schema definitions
│   │   ├── fs.rs            # read_file, write_file, edit_file (surgical replacement)
│   │   ├── diff.rs          # Colored unified diff engine (similar crate)
│   │   ├── bash.rs          # Shell execution with output truncation
│   │   └── search.rs        # Fast glob & grep search
│   ├── repl.rs              # Interactive REPL with Tab autocompletion & history
│   ├── git.rs               # AI Git commit generator & diff reviewer
│   ├── shell.rs             # Natural language shell assistant
│   ├── files.rs             # File attachment & line-range parsing
│   ├── config.rs            # Persistent config management (~/.config/deepseek-cli/)
│   └── ui.rs                # Stream rendering, DeepThink reasoning box, clipboard
└── .github/
    └── workflows/
        └── ci.yml           # Continuous integration (cargo check & build)
```

---

## 2. Core Agent Protocols & Guidelines

When acting as an autonomous agent in this codebase, adhere strictly to these rules:

### A. Read Before Modifying
- Always inspect existing code using `read_file` or search symbols with `grep` before making assumptions.
- Verify file imports, existing patterns, and variable names.

### B. Surgical Editing (`edit_file`)
- **Never rewrite entire files** when modifying existing logic.
- Use `edit_file` with exact `target` and `replacement` strings.
- Include 1–2 lines of surrounding context in `target` to ensure uniqueness.
- Preserve exact whitespace and indentation.

### C. Build & Verification
- After modifying Rust code, **always run verification** via `bash`:
  ```bash
  cargo check
  ```
- Before finalizing a task, verify tests:
  ```bash
  cargo test
  ```
- Ensure zero compiler warnings in production paths.

### D. Subagent Delegation (`spawn_subagent`)
The agent has the power to spawn autonomous subagents with `spawn_subagent`. Use this capability strategically:
- **When to spawn a subagent**:
  - Broad codebase research (e.g. finding all usages across files).
  - Isolated testing or benchmark runs.
  - Multi-step exploratory tasks that would otherwise clutter the primary conversation context.
- **Subagent Roles**:
  - `Code Researcher`: Read and summarize relevant codebase areas.
  - `Test Runner`: Execute test suites and parse failure traces.
  - `Security Auditor`: Check for vulnerabilities or credential leaks.

---

## 3. Tool Reference

| Tool | Purpose | Safe / Mutating |
|---|---|---|
| `read_file` | Read line-numbered file content (max 800 lines/call). | Safe (Auto-approved) |
| `edit_file` | Surgical substring replacement with visual diff. | Mutating (Requires approval) |
| `write_file` | Create or overwrite files. | Mutating (Requires approval) |
| `bash` | Execute shell commands in current workspace. | Mutating (Requires approval) |
| `glob` | Find files matching pattern (ignores `.git`, `target`, `node_modules`). | Safe (Auto-approved) |
| `grep` | Search text or regex patterns across files. | Safe (Auto-approved) |
| `spawn_subagent` | Launch an autonomous child agent for a subtask. | Safe (Spawns isolated subagent) |

---

## 4. Coding Conventions

- **Rust Edition**: 2021.
- **Error Handling**: Use `anyhow::Result` for application code. Propagate errors with `?`. Avoid `unwrap()` or `expect()` in library paths.
- **Async Runtime**: `tokio` with async/await.
- **Formatting**: Adhere to `rustfmt` standard style.
- **Comments**: Keep comments concise and intentional. Do not add redundant commentary for self-explanatory code.

---

## 5. Git & Commit Guidelines

- Follow **Conventional Commits**:
  - `feat(...)`: New feature or capability
  - `fix(...)`: Bug fix
  - `refactor(...)`: Code improvement without behavioral change
  - `docs(...)`: Documentation updates
  - `test(...)`: Unit or integration tests
  - `chore(...)`: Dependencies or build scripts
- Keep commit summary under 72 characters.
