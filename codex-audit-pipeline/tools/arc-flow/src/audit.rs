use anyhow::{Context, Result};
use ignore::WalkBuilder;
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// ============================================================
// 配置结构体（与 .codex/audit.toml 对应）
// ============================================================
#[derive(Debug, Default, Deserialize, Clone)]
struct PathsConfig {
    #[serde(default)]
    exclude: Vec<String>,
    /// 路径别名表，例如 backend = "backend"；规则里写别名即可
    #[serde(flatten)]
    aliases: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
struct Config {
    #[serde(default)]
    paths: PathsConfig,
    #[serde(default)]
    hard_rules: Vec<HardRule>,
    #[serde(default)]
    arch_rules: Vec<ArchRule>,
}

#[derive(Debug, Deserialize, Clone)]
struct HardRule {
    name: String,
    severity: String,
    paths: Vec<String>,
    extensions: Vec<String>,
    patterns: Vec<String>,
    #[serde(default)]
    exclude_patterns: Vec<String>,
    #[serde(default)]
    allowlist: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct ArchRule {
    name: String,
    layer: String,
    paths: Vec<String>,
    extensions: Vec<String>,
    forbidden_patterns: Vec<String>,
    #[serde(default)]
    allowed_patterns: Vec<String>,
    suggestion: String,
    #[serde(default)]
    exclude_patterns: Vec<String>,
    #[serde(default)]
    allowlist: Vec<String>,
}

// ============================================================
// 违规结构体
// ============================================================
#[derive(Debug, Clone)]
struct Violation {
    file: PathBuf,
    line: usize,
    content: String,
    rule_name: String,
}

#[derive(Debug, Clone)]
struct ArchViolation {
    file: PathBuf,
    line: usize,
    content: String,
    rule_name: String,
}

// ============================================================
// 日志解析模块
// ============================================================
mod log_parser {
    use super::*;
    use serde_json::Value;
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    fn extract_trace_id(json: &Value) -> Option<String> {
        json.get("trace_id")
            .or_else(|| json.get("fields").and_then(|f| f.get("trace_id")))
            .or_else(|| json.get("data").and_then(|d| d.get("trace_id")))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn level_of(json: &Value) -> String {
        json.get("level")
            .or_else(|| json.get("severity"))
            .and_then(|v| v.as_str())
            .unwrap_or("INFO")
            .to_uppercase()
    }

    pub fn extract_error_context(input_path: &str, output_path: &str) -> Result<()> {
        let file = File::open(input_path)?;
        let reader = BufReader::new(file);

        let mut error_trace_id = String::new();
        let mut last_trace_id = String::new();
        let mut structured_logs: Vec<Value> = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(json) = serde_json::from_str::<Value>(&line) {
                if let Some(tid) = extract_trace_id(&json) {
                    last_trace_id = tid.clone();
                    // 优先取第一条 ERROR 日志所在的 trace_id（比"最后一条"可靠）
                    if error_trace_id.is_empty() && level_of(&json) == "ERROR" {
                        error_trace_id = tid;
                    }
                }
                structured_logs.push(json);
            }
        }

        let target_trace_id = if error_trace_id.is_empty() {
            last_trace_id
        } else {
            error_trace_id
        };

        if target_trace_id.is_empty() {
            eprintln!("⚠️ 未找到 trace_id，降级输出原始日志尾部 30 行");
            let last_lines = get_last_n_lines(input_path, 30)?;
            fs::write(output_path, last_lines)?;
            return Ok(());
        }

        let mut output = Vec::new();
        for log in &structured_logs {
            if extract_trace_id(log).as_deref() != Some(target_trace_id.as_str()) {
                continue;
            }
            let timestamp = log
                .get("timestamp")
                .or_else(|| log.get("time"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let level = level_of(log);
            let target = log
                .get("target")
                .or_else(|| log.get("module"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let fields = log.get("fields").or_else(|| log.get("data"));
            let msg = fields
                .and_then(|f| f.get("message").or_else(|| f.get("msg")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let error = fields
                .and_then(|f| f.get("error"))
                .or_else(|| log.get("error"))
                .map(|v| {
                    if let Some(s) = v.as_str() {
                        s.to_string()
                    } else {
                        v.to_string()
                    }
                })
                .unwrap_or_default();

            let compact = serde_json::json!({
                "timestamp": timestamp,
                "level": level,
                "target": target,
                "msg": msg,
                "error": error,
                "trace_id": target_trace_id,
            });
            output.push(compact);
        }

        // 只保留最相关的上下文（避免超出 LLM 上下文窗口）
        if output.len() > 30 {
            output.truncate(30);
        }

        let json_output = serde_json::to_string_pretty(&output)?;
        fs::write(output_path, json_output)?;
        eprintln!(
            "✅ 结构化日志已提取: {} ({} 条, trace_id={})",
            output_path,
            output.len(),
            target_trace_id
        );
        Ok(())
    }

    fn get_last_n_lines(path: &str, n: usize) -> Result<String> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().collect::<std::result::Result<Vec<_>, _>>()?;
        let start = lines.len().saturating_sub(n);
        Ok(lines[start..].join("\n"))
    }
}

// ============================================================
// 核心扫描函数
// ============================================================
fn compile_regexes(patterns: &[String]) -> Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|pattern| {
            Regex::new(pattern).with_context(|| format!("invalid audit regex {pattern:?}"))
        })
        .collect()
}

/// 将规则中的路径条目解析为实际目录：命中别名表则替换，否则原样使用。
fn resolve_root_dirs(entries: &[String], aliases: &HashMap<String, String>) -> Vec<String> {
    entries
        .iter()
        .map(|e| aliases.get(e).cloned().unwrap_or_else(|| e.clone()))
        .collect()
}

/// 判断 match 是否落在行注释（// 或 ///）之内。
/// 说明：多行块注释 /* */ 不做处理（README 已注明为已知限制）。
fn is_inside_line_comment(line: &str, match_start: usize) -> bool {
    line.find("//").is_some_and(|pos| pos < match_start)
}

fn is_allowlisted(path: &Path, allowlist: &[String]) -> bool {
    let path_str = path.to_str().unwrap_or("");
    allowlist.iter().any(|pattern| {
        if pattern.contains('*') || pattern.contains('[') || pattern.contains('(') {
            Regex::new(pattern)
                .map(|re| re.is_match(path_str))
                .unwrap_or(false)
        } else {
            path.starts_with(Path::new(pattern))
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn scan_files(
    root_dirs: &[String],
    extensions: &[String],
    exclude_dirs: &[String],
    patterns: &[String],
    exclude_patterns: &[String],
    allowlist: &[String],
    rule_name: &str,
) -> Result<Vec<Violation>> {
    if root_dirs.is_empty() || patterns.is_empty() {
        return Ok(Vec::new());
    }

    let root_paths: Vec<PathBuf> = root_dirs
        .iter()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .collect();
    let exclude_set: Vec<PathBuf> = exclude_dirs.iter().map(PathBuf::from).collect();
    let regexes = compile_regexes(patterns)?;
    let exclude_regexes = compile_regexes(exclude_patterns)?;

    if root_paths.is_empty() {
        return Ok(Vec::new());
    }

    let rule_name = rule_name.to_string();
    let mut walk_builder = WalkBuilder::new(root_paths[0].clone());
    for root_path in root_paths.iter().skip(1) {
        walk_builder.add(root_path);
    }
    let violations = walk_builder
        .add_custom_ignore_filename(".auditignore")
        .follow_links(false)
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let path = entry.path();
            if path.is_dir() {
                return false;
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if !extensions.contains(&ext.to_string()) {
                    return false;
                }
            } else {
                return false;
            }
            for excl in &exclude_set {
                if path.starts_with(excl) {
                    return false;
                }
            }
            let path_str = path.to_str().unwrap_or("");
            if exclude_regexes.iter().any(|re| re.is_match(path_str)) {
                return false;
            }
            !is_allowlisted(path, allowlist)
        })
        .par_bridge()
        .filter_map(|entry| {
            let path = entry.path();
            let content = fs::read_to_string(path).ok()?;
            let mut violations = Vec::new();

            for (line_num, line) in content.lines().enumerate() {
                for (idx, re) in regexes.iter().enumerate() {
                    let mut found = false;
                    for m in re.find_iter(line) {
                        if is_inside_line_comment(line, m.start()) {
                            continue;
                        }
                        violations.push(Violation {
                            file: path.to_path_buf(),
                            line: line_num + 1,
                            content: line.trim().to_string(),
                            rule_name: format!("{}:{}", rule_name, patterns[idx]),
                        });
                        found = true;
                        break;
                    }
                    if found {
                        break;
                    }
                }
            }
            if violations.is_empty() {
                None
            } else {
                Some(violations)
            }
        })
        .flatten()
        .collect();
    Ok(violations)
}

fn scan_arch_rules(config: &Config) -> Result<Vec<ArchViolation>> {
    let mut all_violations = Vec::new();

    for rule in &config.arch_rules {
        let root_dirs = resolve_root_dirs(&rule.paths, &config.paths.aliases);
        let extensions = rule.extensions.clone();
        let exclude_dirs = config.paths.exclude.clone();
        let patterns = rule.forbidden_patterns.clone();
        let allowed_patterns = rule.allowed_patterns.clone();
        let exclude_patterns = rule.exclude_patterns.clone();
        let allowlist = rule.allowlist.clone();

        if root_dirs.is_empty() || patterns.is_empty() {
            continue;
        }

        let root_paths: Vec<PathBuf> = root_dirs
            .iter()
            .map(PathBuf::from)
            .filter(|path| path.exists())
            .collect();
        let exclude_set: Vec<PathBuf> = exclude_dirs.iter().map(PathBuf::from).collect();
        let regexes = compile_regexes(&patterns)?;
        let allowed_regexes = compile_regexes(&allowed_patterns)?;
        let exclude_regexes = compile_regexes(&exclude_patterns)?;

        if root_paths.is_empty() {
            continue;
        }

        let rule_name = rule.name.clone();
        let mut walk_builder = WalkBuilder::new(root_paths[0].clone());
        for root_path in root_paths.iter().skip(1) {
            walk_builder.add(root_path);
        }
        let rule_violations: Vec<ArchViolation> = walk_builder
            .add_custom_ignore_filename(".auditignore")
            .follow_links(false)
            .build()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let path = entry.path();
                if path.is_dir() {
                    return false;
                }
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if !extensions.contains(&ext.to_string()) {
                        return false;
                    }
                } else {
                    return false;
                }
                for excl in &exclude_set {
                    if path.starts_with(excl) {
                        return false;
                    }
                }
                let path_str = path.to_str().unwrap_or("");
                if exclude_regexes.iter().any(|re| re.is_match(path_str)) {
                    return false;
                }
                !is_allowlisted(path, &allowlist)
            })
            .par_bridge()
            .filter_map(|entry| {
                let path = entry.path();
                let content = fs::read_to_string(path).ok()?;
                let mut violations = Vec::new();

                for (line_num, line) in content.lines().enumerate() {
                    for re in &regexes {
                        let mut found = false;
                        for m in re.find_iter(line) {
                            if is_inside_line_comment(line, m.start()) {
                                continue;
                            }
                            if allowed_regexes.iter().any(|allowed| allowed.is_match(line)) {
                                continue;
                            }
                            violations.push(ArchViolation {
                                file: path.to_path_buf(),
                                line: line_num + 1,
                                content: line.trim().to_string(),
                                rule_name: rule_name.clone(),
                            });
                            found = true;
                            break;
                        }
                        if found {
                            break;
                        }
                    }
                }
                if violations.is_empty() {
                    None
                } else {
                    Some(violations)
                }
            })
            .flatten()
            .collect();

        all_violations.extend(rule_violations);
    }

    Ok(all_violations)
}

// ============================================================
// 报告生成
// ============================================================
fn generate_markdown(
    config: &Config,
    hard_violations: &[Violation],
    arch_violations: &[ArchViolation],
) -> String {
    let mut output = String::new();

    output.push_str("=== 【自动化硬性约束扫描结果】 ===\n\n");

    for rule in &config.hard_rules {
        let rule_violations: Vec<&Violation> = hard_violations
            .iter()
            .filter(|v| v.rule_name.starts_with(&rule.name))
            .collect();

        let count = rule_violations.len();
        output.push_str(&format!(">> {}: 违规数量 {}\n", rule.name, count));

        if count > 0 {
            for v in rule_violations.iter().take(3) {
                output.push_str(&format!(
                    "    {}:{}: {}\n",
                    v.file.display(),
                    v.line,
                    v.content
                ));
            }
            if count > 3 {
                output.push_str(&format!("    ... 剩余 {} 处\n", count - 3));
            }
        } else {
            output.push_str("  ✅ 未发现\n");
        }
        output.push('\n');
    }

    output.push_str("=== 【架构分层违规预扫描】 ===\n\n");

    for rule in &config.arch_rules {
        let violations: Vec<&ArchViolation> = arch_violations
            .iter()
            .filter(|v| v.rule_name == rule.name)
            .collect();
        let count = violations.len();

        output.push_str(&format!(">> {}: 违规数量 {}\n", rule.name, count));

        if count > 0 {
            for v in violations.iter().take(3) {
                output.push_str(&format!(
                    "    {}:{}: {}\n",
                    v.file.display(),
                    v.line,
                    v.content
                ));
            }
            if count > 3 {
                output.push_str(&format!("    ... 剩余 {} 处\n", count - 3));
            }
            output.push_str(&format!("  💡 建议: {}\n", rule.suggestion));
        } else {
            output.push_str("  ✅ 未发现违规\n");
        }
        output.push('\n');
    }

    output
}

#[derive(Debug, Serialize)]
struct JsonOccurrence {
    file: String,
    line: usize,
    content: String,
}

#[derive(Debug, Serialize)]
struct JsonViolation {
    rule: String,
    severity: String,
    count: usize,
    occurrences: Vec<JsonOccurrence>,
}

#[derive(Debug, Serialize)]
struct JsonArchViolation {
    rule: String,
    layer: String,
    count: usize,
    suggestion: String,
    occurrences: Vec<JsonOccurrence>,
}

#[derive(Debug, Clone, Serialize)]
struct JsonSummary {
    total_violations: usize,
    blocker_count: usize,
    error_count: usize,
}

#[derive(Debug, Serialize)]
struct JsonReport {
    timestamp: String,
    hard_violations: Vec<JsonViolation>,
    arch_violations: Vec<JsonArchViolation>,
    summary: JsonSummary,
}

fn generate_report(
    config: &Config,
    hard_violations: &[Violation],
    arch_violations: &[ArchViolation],
) -> JsonReport {
    let mut hard_json = Vec::new();
    for rule in &config.hard_rules {
        let rule_violations: Vec<&Violation> = hard_violations
            .iter()
            .filter(|v| v.rule_name.starts_with(&rule.name))
            .collect();

        let occurrences: Vec<JsonOccurrence> = rule_violations
            .iter()
            .map(|v| JsonOccurrence {
                file: v.file.to_string_lossy().to_string(),
                line: v.line,
                content: v.content.clone(),
            })
            .collect();

        hard_json.push(JsonViolation {
            rule: rule.name.clone(),
            severity: rule.severity.clone(),
            count: occurrences.len(),
            occurrences,
        });
    }

    let mut arch_json = Vec::new();
    for rule in &config.arch_rules {
        let rule_violations: Vec<&ArchViolation> = arch_violations
            .iter()
            .filter(|v| v.rule_name == rule.name)
            .collect();

        let occurrences: Vec<JsonOccurrence> = rule_violations
            .iter()
            .map(|v| JsonOccurrence {
                file: v.file.to_string_lossy().to_string(),
                line: v.line,
                content: v.content.clone(),
            })
            .collect();

        arch_json.push(JsonArchViolation {
            rule: rule.name.clone(),
            layer: rule.layer.clone(),
            count: occurrences.len(),
            suggestion: rule.suggestion.clone(),
            occurrences,
        });
    }

    let total: usize = hard_json.iter().map(|v| v.count).sum::<usize>()
        + arch_json.iter().map(|v| v.count).sum::<usize>();
    let blocker_count: usize = hard_json
        .iter()
        .filter(|v| v.severity == "blocker")
        .map(|v| v.count)
        .sum();
    let error_count: usize = hard_json
        .iter()
        .filter(|v| v.severity == "error")
        .map(|v| v.count)
        .sum();

    JsonReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        hard_violations: hard_json,
        arch_violations: arch_json,
        summary: JsonSummary {
            total_violations: total,
            blocker_count,
            error_count,
        },
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditOutcome {
    pub total_violations: usize,
    pub blocker_count: usize,
    pub error_count: usize,
}

pub fn run(config_path: &Path, report_dir: &Path, emit_json: bool) -> Result<AuditOutcome> {
    let config_str = fs::read_to_string(config_path)
        .with_context(|| format!("read audit config {}", config_path.display()))?;
    let config: Config = toml::from_str(&config_str)
        .with_context(|| format!("parse audit config {}", config_path.display()))?;

    let mut all_hard_violations = Vec::new();
    for rule in &config.hard_rules {
        let root_dirs = resolve_root_dirs(&rule.paths, &config.paths.aliases);
        let violations = scan_files(
            &root_dirs,
            &rule.extensions,
            &config.paths.exclude,
            &rule.patterns,
            &rule.exclude_patterns,
            &rule.allowlist,
            &rule.name,
        )?;
        all_hard_violations.extend(violations);
    }

    let arch_violations = scan_arch_rules(&config)?;
    let report = generate_report(&config, &all_hard_violations, &arch_violations);
    let full_json = serde_json::to_string_pretty(&report)?;
    let outcome = AuditOutcome {
        total_violations: report.summary.total_violations,
        blocker_count: report.summary.blocker_count,
        error_count: report.summary.error_count,
    };

    fs::create_dir_all(report_dir)?;
    fs::write(report_dir.join("review_context.json"), &full_json)?;

    let markdown = generate_markdown(&config, &all_hard_violations, &arch_violations);
    let truncated = if markdown.len() > 4096 {
        let mut value = markdown;
        let mut boundary = 4096;
        while !value.is_char_boundary(boundary) {
            boundary -= 1;
        }
        value.truncate(boundary);
        value.push_str("\n\n... (report truncated to 4KB; see review_context.json)");
        value
    } else {
        markdown
    };
    fs::write(report_dir.join("review_context.md"), truncated)?;

    if emit_json {
        println!("{full_json}");
    }

    Ok(outcome)
}

pub fn parse_logs(input: &Path, output: &Path) -> Result<()> {
    log_parser::extract_error_context(&input.to_string_lossy(), &output.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time must be after Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "arc-flow-auditor-{name}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn child(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir_all(&path).expect("create child directory");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn hard_rule_scans_every_configured_root() {
        let test_dir = TestDir::new("hard-roots");
        let first = test_dir.child("first");
        let second = test_dir.child("second");
        fs::write(first.join("one.rs"), "forbidden_call();\n").expect("write first fixture");
        fs::write(second.join("two.rs"), "forbidden_call();\n").expect("write second fixture");

        let roots = vec![
            first.to_string_lossy().into_owned(),
            second.to_string_lossy().into_owned(),
        ];
        let violations = scan_files(
            &roots,
            &["rs".to_string()],
            &[],
            &["forbidden_call".to_string()],
            &[],
            &[],
            "test rule",
        )
        .expect("scan fixture");

        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn architecture_rule_scans_every_configured_root() {
        let test_dir = TestDir::new("arch-roots");
        let pages = test_dir.child("pages");
        let layout = test_dir.child("layout");
        fs::write(pages.join("page.ts"), "HttpClient\n").expect("write page fixture");
        fs::write(layout.join("layout.ts"), "HttpClient\n").expect("write layout fixture");

        let config = Config {
            paths: PathsConfig {
                exclude: Vec::new(),
                aliases: HashMap::new(),
            },
            hard_rules: Vec::new(),
            arch_rules: vec![ArchRule {
                name: "component rule".to_string(),
                layer: "component".to_string(),
                paths: vec![
                    pages.to_string_lossy().into_owned(),
                    layout.to_string_lossy().into_owned(),
                ],
                extensions: vec!["ts".to_string()],
                forbidden_patterns: vec!["HttpClient".to_string()],
                allowed_patterns: Vec::new(),
                suggestion: "use a service".to_string(),
                exclude_patterns: Vec::new(),
                allowlist: Vec::new(),
            }],
        };

        assert_eq!(scan_arch_rules(&config).expect("scan config").len(), 2);
    }

    #[test]
    fn literal_allowlist_is_a_path_prefix_not_a_substring() {
        let allowlist = vec!["backend/src/repositories".to_string()];

        assert!(is_allowlisted(
            Path::new("backend/src/repositories/users.rs"),
            &allowlist
        ));
        assert!(!is_allowlisted(
            Path::new("backend/src/repositories_backup/users.rs"),
            &allowlist
        ));
    }

    #[test]
    fn invalid_rule_regex_returns_an_error() {
        let error = compile_regexes(&["(".to_string()]).expect_err("invalid regex must fail");
        assert!(error.to_string().contains("invalid audit regex"));
    }

    #[test]
    fn log_parser_keeps_the_error_trace() {
        let test_dir = TestDir::new("parse-logs");
        let input = test_dir.0.join("input.jsonl");
        let output = test_dir.0.join("output.json");
        fs::write(
            &input,
            concat!(
                "{\"level\":\"INFO\",\"trace_id\":\"failed\",\"fields\":{\"message\":\"start\"}}\n",
                "{\"level\":\"ERROR\",\"trace_id\":\"failed\",\"fields\":{\"error\":\"root cause\"}}\n",
                "{\"level\":\"INFO\",\"trace_id\":\"other\",\"fields\":{\"message\":\"later\"}}\n"
            ),
        )
        .expect("write log fixture");

        log_parser::extract_error_context(&input.to_string_lossy(), &output.to_string_lossy())
            .expect("parse logs");
        let parsed: Vec<serde_json::Value> =
            serde_json::from_slice(&fs::read(output).expect("read output")).expect("output JSON");

        assert_eq!(parsed.len(), 2);
        assert!(parsed.iter().all(|entry| entry["trace_id"] == "failed"));
    }
}
