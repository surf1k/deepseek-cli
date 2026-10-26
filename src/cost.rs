use crate::api::GenerationStats;
use colored::*;

pub struct CostBreakdown {
    pub actual_cost_usd: f64,
    pub claude_cost_usd: f64,
    pub savings_usd: f64,
}

pub fn calculate_cost(stats: &GenerationStats, model: &str) -> CostBreakdown {
    let is_r1 = model.contains("reasoner") || model.contains("r1");

    // DeepSeek pricing per token (USD)
    // V3: Cache Hit: $0.14/M, Cache Miss: $0.28/M, Output: $2.19/M
    // R1: Cache Hit: $0.14/M, Cache Miss: $0.55/M, Output: $2.19/M
    let cache_hit_rate = 0.14 / 1_000_000.0;
    let cache_miss_rate = if is_r1 { 0.55 / 1_000_000.0 } else { 0.28 / 1_000_000.0 };
    let output_rate = 2.19 / 1_000_000.0;

    let hit_tokens = stats.cache_hit_tokens as f64;
    let miss_tokens = if stats.cache_miss_tokens > 0 {
        stats.cache_miss_tokens as f64
    } else {
        (stats.prompt_tokens.saturating_sub(stats.cache_hit_tokens)) as f64
    };
    let output_tokens = stats.completion_tokens as f64;

    let actual_cost_usd = (hit_tokens * cache_hit_rate)
        + (miss_tokens * cache_miss_rate)
        + (output_tokens * output_rate);

    // Claude 3.5 Sonnet pricing: Input: $3.00/M, Output: $15.00/M
    let claude_input_rate = 3.00 / 1_000_000.0;
    let claude_output_rate = 15.00 / 1_000_000.0;
    let claude_cost_usd = ((hit_tokens + miss_tokens) * claude_input_rate)
        + (output_tokens * claude_output_rate);

    let savings_usd = (claude_cost_usd - actual_cost_usd).max(0.0);

    CostBreakdown {
        actual_cost_usd,
        claude_cost_usd,
        savings_usd,
    }
}

pub fn format_cost_line(stats: &GenerationStats, model: &str) -> String {
    let cost = calculate_cost(stats, model);
    if cost.actual_cost_usd < 0.0001 && cost.actual_cost_usd > 0.0 {
        format!(
            "💰 Cost: {} (Saved {} vs Claude 3.5)",
            format!("${:.5}", cost.actual_cost_usd).green().bold(),
            format!("${:.4}", cost.savings_usd).cyan()
        )
    } else {
        format!(
            "💰 Cost: {} (Saved {} vs Claude 3.5)",
            format!("${:.4}", cost.actual_cost_usd).green().bold(),
            format!("${:.3}", cost.savings_usd).cyan()
        )
    }
}

pub fn print_detailed_cost(stats: &GenerationStats, model: &str) {
    let cost = calculate_cost(stats, model);
    println!("{}", "Session Economics & Cost Breakdown:".bold().cyan());
    println!("  Model used:          {}", model.bold());
    println!("  Cache hit tokens:    {} tokens ($0.14/1M)", stats.cache_hit_tokens);
    println!("  Cache miss tokens:   {} tokens", stats.cache_miss_tokens);
    println!("  Completion tokens:   {} tokens ($2.19/1M)", stats.completion_tokens);
    println!("  Total tokens:        {}", stats.total_tokens);
    println!("  ---------------------------------------------");
    println!("  DeepSeek API Cost:   {}", format!("${:.5}", cost.actual_cost_usd).green().bold());
    println!("  Claude 3.5 Cost:     {}", format!("${:.4}", cost.claude_cost_usd).dimmed());
    println!("  Money Saved:         {} 🚀", format!("${:.4}", cost.savings_usd).cyan().bold());
}
