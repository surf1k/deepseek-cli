use crate::config::Config;
use crate::ui::create_spinner;
use anyhow::{Context, Result};
use colored::*;
use std::io::{stdin, stdout, Write};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

pub fn open_browser(url: &str) -> bool {
    if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("cmd");
        c.args(["/c", "start", url]);
        return c.spawn().is_ok();
    }

    let bin = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };

    let mut cmd = std::process::Command::new(bin);
    cmd.arg(url);
    if let Ok(null_out) = std::fs::File::create("/dev/null") {
        cmd.stdout(null_out);
    }
    if let Ok(null_err) = std::fs::File::create("/dev/null") {
        cmd.stderr(null_err);
    }
    cmd.spawn().is_ok()
}

pub fn clean_key_input(raw: &str) -> String {
    raw.trim()
        .trim_matches(|c| c == '\'' || c == '"' || c == '`')
        .trim()
        .to_string()
}

pub async fn run_login_command(cfg: &mut Config) -> Result<()> {
    println!("{}", "DeepSeek CLI Authentication".bold().cyan());
    let key = prompt_authentication(cfg).await?;
    cfg.api_key = Some(key);
    cfg.save()?;
    println!("{} Logged in and credentials saved successfully.", "✓".green().bold());
    Ok(())
}

pub async fn prompt_authentication(cfg: &mut Config) -> Result<String> {
    println!(
        "{}",
        "╭── Authentication Method ─────────────────────────────────────────────╮".cyan().bold()
    );
    println!(
        "│ {} {}                                │",
        "[1]".yellow().bold(),
        "Web Browser Login (Recommended)            ".bold()
    );
    println!("│     • Opens DeepSeek Platform to log in and copy your API key        │");
    println!("│     • Local server captures key automatically via web page           │");
    println!("│                                                                      │");
    println!(
        "│ {} {}                                │",
        "[2]".yellow().bold(),
        "Enter API Key directly                     ".bold()
    );
    println!("│     • Paste your key directly in the terminal (sk-...)               │");
    println!("│                                                                      │");
    println!(
        "│ {} {}                                │",
        "[3]".yellow().bold(),
        "Custom Endpoint (Ollama / OpenRouter / vLLM)".bold()
    );
    println!("│     • Specify a custom Base URL and local/remote model               │");
    println!(
        "{}",
        "╰──────────────────────────────────────────────────────────────────────╯".cyan().bold()
    );

    print!("\n{} ", "❯ Select option [1/2/3, default: 1]:".bold());
    let _ = stdout().flush();

    let mut choice = String::new();
    let _ = stdin().read_line(&mut choice);

    match choice.trim() {
        "2" => prompt_key_terminal(cfg).await,
        "3" => prompt_custom_endpoint(cfg).await,
        _ => prompt_web_browser_login(cfg).await,
    }
}

async fn prompt_web_browser_login(cfg: &Config) -> Result<String> {
    // 1. Try binding local port 18981, or fallback to any free port (0)
    let listener = match TcpListener::bind("127.0.0.1:18981").await {
        Ok(l) => l,
        Err(_) => TcpListener::bind("127.0.0.1:0")
            .await
            .context("Failed to bind local loopback server for web auth")?,
    };

    let port = listener.local_addr()?.port();
    let local_url = format!("http://localhost:{}", port);
    let (tx, mut rx) = oneshot::channel();
    let mut rx_done = false;

    // 2. Spawn background local HTTP server
    let server_task = tokio::spawn(async move {
        run_local_server(listener, tx).await;
    });

    println!("\n  {} Starting local authentication helper on {}", "🌐".cyan(), local_url.bold().green());
    println!(
        "  {} DeepSeek Platform: {}",
        "→".dimmed(),
        "https://platform.deepseek.com/api_keys".underline().blue()
    );

    // 3. Launch default browser
    let opened = open_browser(&local_url);
    if opened {
        println!("  {} Browser opened automatically.", "✓".green());
    } else {
        println!(
            "  {} Please open {} in your browser.",
            "ℹ".yellow(),
            local_url.bold().cyan()
        );
    }

    println!("\n  {}", "Waiting for authentication:".bold());
    println!("  • Complete login in the browser tab, OR");
    println!("  • Paste your API Key directly below\n");

    loop {
        print!("{} ", "❯ Enter API Key (sk-...):".bold().yellow());
        let _ = stdout().flush();

        let raw_key = if !rx_done {
            tokio::select! {
                browser_res = &mut rx => {
                    rx_done = true;
                    if let Ok(web_key) = browser_res {
                        println!("\n  {} Key received from browser!", "✓".green().bold());
                        web_key
                    } else {
                        read_terminal_line().await?
                    }
                }
                term_line = read_terminal_line() => {
                    term_line?
                }
            }
        } else {
            read_terminal_line().await?
        };

        let key = clean_key_input(&raw_key);
        if key.is_empty() {
            println!("  {} API key cannot be empty. Please try again.", "✗".red());
            continue;
        }

        // Validate Key
        let spinner = create_spinner("Verifying API key with DeepSeek servers...");
        let is_valid = validate_key(&cfg.base_url, &key).await;
        spinner.finish_and_clear();

        // Abort background server task now that we have a key
        server_task.abort();

        match is_valid {
            Some(true) => {
                println!(
                    "  {} API key verified! Connected to DeepSeek Platform.",
                    "✓".green().bold()
                );
                return Ok(key);
            }
            Some(false) => {
                println!(
                    "  {} DeepSeek server rejected this key (401 Unauthorized).",
                    "⚠".yellow().bold()
                );
                print!("  Do you want to retry entering your key? [Y/n]: ");
                let _ = stdout().flush();
                let mut retry = String::new();
                let _ = stdin().read_line(&mut retry);
                if retry.trim().eq_ignore_ascii_case("n") {
                    return Ok(key);
                }
            }
            None => {
                println!(
                    "  {} Could not verify key online (offline/custom network). Proceeding.",
                    "ℹ".cyan()
                );
                return Ok(key);
            }
        }
    }
}

async fn prompt_key_terminal(cfg: &Config) -> Result<String> {
    println!(
        "\n{}",
        "Get your key from: https://platform.deepseek.com/api_keys".underline().blue()
    );

    loop {
        print!("\n{} ", "❯ Enter your API Key (sk-...):".bold().yellow());
        let _ = stdout().flush();

        let mut input = String::new();
        stdin().read_line(&mut input)?;
        let key = clean_key_input(&input);

        if key.is_empty() {
            println!("  {} API key cannot be empty.", "✗".red());
            continue;
        }

        let spinner = create_spinner("Verifying API key with DeepSeek servers...");
        let is_valid = validate_key(&cfg.base_url, &key).await;
        spinner.finish_and_clear();

        match is_valid {
            Some(true) => {
                println!("  {} API key verified successfully!", "✓".green().bold());
                return Ok(key);
            }
            Some(false) => {
                println!(
                    "  {} DeepSeek rejected this key (401 Unauthorized).",
                    "⚠".yellow().bold()
                );
                print!("  Use this key anyway? [y/N]: ");
                let _ = stdout().flush();
                let mut confirm = String::new();
                let _ = stdin().read_line(&mut confirm);
                if confirm.trim().eq_ignore_ascii_case("y") {
                    return Ok(key);
                }
            }
            None => {
                println!("  {} Server unreachable. Using key without verification.", "ℹ".cyan());
                return Ok(key);
            }
        }
    }
}

async fn prompt_custom_endpoint(cfg: &mut Config) -> Result<String> {
    println!("\n{}", "Custom Endpoint Setup:".bold().cyan());
    print!("❯ Base URL [default: {}]: ", cfg.base_url);
    let _ = stdout().flush();
    let mut url_input = String::new();
    stdin().read_line(&mut url_input)?;
    let url = url_input.trim();
    if !url.is_empty() {
        cfg.base_url = url.to_string();
    }

    print!("❯ API Key (optional, press Enter to skip): ");
    let _ = stdout().flush();
    let mut key_input = String::new();
    stdin().read_line(&mut key_input)?;
    let key = clean_key_input(&key_input);

    let final_key = if key.is_empty() {
        "custom-endpoint-key".to_string()
    } else {
        key
    };

    println!("{} Custom endpoint configured.", "✓".green());
    Ok(final_key)
}

async fn read_terminal_line() -> Result<String> {
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    Ok(line)
}

pub async fn validate_key(base_url: &str, key: &str) -> Option<bool> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .ok()?;

    let res = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", key))
        .send()
        .await
        .ok()?;

    Some(res.status().is_success())
}

async fn run_local_server(listener: TcpListener, tx: oneshot::Sender<String>) {
    let mut tx_opt = Some(tx);

    while let Ok((mut socket, _)) = listener.accept().await {
        let mut total_req = Vec::new();
        let mut buf = [0u8; 4096];
        let mut content_len: Option<usize> = None;
        let mut header_end: Option<usize> = None;

        while let Ok(n) = socket.read(&mut buf).await {
            if n == 0 {
                break;
            }
            total_req.extend_from_slice(&buf[..n]);

            if header_end.is_none() {
                if let Some(pos) = total_req.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = Some(pos + 4);
                    let header_str = String::from_utf8_lossy(&total_req[..pos]);
                    for line in header_str.lines() {
                        if line.to_lowercase().starts_with("content-length:") {
                            if let Some(val) = line.split(':').nth(1) {
                                content_len = val.trim().parse().ok();
                            }
                        }
                    }
                }
            }

            if let Some(end) = header_end {
                let current_body_len = total_req.len() - end;
                let needed = content_len.unwrap_or(0);
                if current_body_len >= needed {
                    break;
                }
            }
        }

        let req = String::from_utf8_lossy(&total_req);

        if req.starts_with("GET /") && !req.starts_with("GET /submit") {
            let html = get_auth_html();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.len(),
                html
            );
            let _ = socket.write_all(resp.as_bytes()).await;
            let _ = socket.flush().await;
        } else if req.starts_with("POST /submit") || req.starts_with("GET /submit") {
            // Check query param first: /submit?key=...
            let mut extracted_key = String::new();
            if let Some(q_idx) = req.find("?key=") {
                let after = &req[q_idx + 5..];
                let end = after.find(' ').unwrap_or(after.len());
                extracted_key = url_decode(&after[..end]);
            }

            // If not found in query, check body
            if extracted_key.is_empty() {
                if let Some(end) = header_end {
                    let body = &req[end..];
                    if let Some(stripped) = body.strip_prefix("key=") {
                        extracted_key = url_decode(stripped);
                    } else {
                        extracted_key = body.trim().to_string();
                    }
                }
            }

            let json_resp = r#"{"status":"ok"}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                json_resp.len(),
                json_resp
            );
            let _ = socket.write_all(resp.as_bytes()).await;
            let _ = socket.flush().await;

            if !extracted_key.is_empty() {
                if let Some(sender) = tx_opt.take() {
                    let _ = sender.send(extracted_key);
                }
                break;
            }
        }
    }
}

fn url_decode(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(b) = u8::from_str_radix(&hex, 16) {
                result.push(b as char);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result.trim().to_string()
}

fn get_auth_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>DeepSeek CLI — Web Authentication</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background: #0b1120;
      color: #f1f5f9;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      display: flex;
      justify-content: center;
      align-items: center;
      min-height: 100vh;
      padding: 20px;
    }
    .card {
      background: #1e293b;
      border: 1px solid #334155;
      border-radius: 16px;
      padding: 36px;
      max-width: 480px;
      width: 100%;
      box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.5);
    }
    .logo {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 20px;
    }
    .logo-badge {
      background: linear-gradient(135deg, #06b6d4, #3b82f6);
      width: 42px;
      height: 42px;
      border-radius: 10px;
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 22px;
      font-weight: bold;
      color: white;
    }
    h1 { font-size: 22px; font-weight: 700; color: #f8fafc; }
    p.sub { font-size: 14px; color: #94a3b8; margin-top: 4px; }
    .btn-keys {
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
      background: #0284c7;
      color: white;
      text-decoration: none;
      padding: 12px 18px;
      border-radius: 8px;
      font-weight: 600;
      font-size: 14px;
      margin: 24px 0 18px 0;
      transition: background 0.2s;
    }
    .btn-keys:hover { background: #0369a1; }
    .divider {
      display: flex;
      align-items: center;
      text-align: center;
      color: #64748b;
      font-size: 12px;
      margin: 18px 0;
    }
    .divider::before, .divider::after {
      content: '';
      flex: 1;
      border-bottom: 1px solid #334155;
    }
    .divider::before { margin-right: 10px; }
    .divider::after { margin-left: 10px; }
    label { font-size: 13px; font-weight: 600; color: #cbd5e1; display: block; margin-bottom: 8px; }
    input[type="text"], input[type="password"] {
      width: 100%;
      background: #0f172a;
      border: 1px solid #334155;
      color: #f8fafc;
      padding: 12px 14px;
      border-radius: 8px;
      font-size: 14px;
      outline: none;
      font-family: monospace;
      transition: border-color 0.2s;
    }
    input:focus { border-color: #38bdf8; }
    .btn-submit {
      width: 100%;
      background: #10b981;
      color: white;
      border: none;
      padding: 12px;
      border-radius: 8px;
      font-weight: 600;
      font-size: 15px;
      cursor: pointer;
      margin-top: 16px;
      transition: background 0.2s;
    }
    .btn-submit:hover { background: #059669; }
    .success-box {
      display: none;
      text-align: center;
      padding: 24px 0;
    }
    .success-icon { font-size: 48px; margin-bottom: 12px; color: #34d399; }
    .success-title { font-size: 20px; font-weight: 700; color: #34d399; margin-bottom: 8px; }
    .success-desc { font-size: 14px; color: #94a3b8; }
  </style>
</head>
<body>
  <div class="card">
    <div class="logo">
      <div class="logo-badge">⚡</div>
      <div>
        <h1>DeepSeek CLI</h1>
        <p class="sub">Authorize terminal coding partner</p>
      </div>
    </div>
    <div id="auth-form-container">
      <a href="https://platform.deepseek.com/api_keys" target="_blank" class="btn-keys">
        🔑 Open DeepSeek Platform to Copy Key ↗
      </a>
      <div class="divider">THEN PASTE YOUR KEY</div>
      <form id="auth-form">
        <label for="apikey">DeepSeek API Key (sk-...)</label>
        <input type="password" id="apikey" name="apikey" placeholder="sk-..." required autocomplete="off" />
        <button type="submit" class="btn-submit">Complete Authentication ✓</button>
      </form>
    </div>
    <div id="success-container" class="success-box">
      <div class="success-icon">✓</div>
      <div class="success-title">Authentication Successful!</div>
      <div class="success-desc">You can now close this tab and return to your terminal. DeepSeek CLI is ready to code.</div>
    </div>
  </div>
  <script>
    const form = document.getElementById('auth-form');
    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      const key = document.getElementById('apikey').value.trim();
      if (!key) return;
      try {
        await fetch('/submit?key=' + encodeURIComponent(key), {
          method: 'POST',
          headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
          body: 'key=' + encodeURIComponent(key)
        });
        document.getElementById('auth-form-container').style.display = 'none';
        document.getElementById('success-container').style.display = 'block';
      } catch (err) {
        alert('Failed to connect to local CLI: ' + err);
      }
    });
  </script>
</body>
</html>"#
}
