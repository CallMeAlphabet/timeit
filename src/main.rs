#![allow(dead_code)]
use std::process::Command;
use std::time::Duration;
use std::thread;
use clihelp::{ColorWhen, HelpPage, Row, Section};

#[derive(Clone, Copy)]
enum DisplayMode {
    Auto,       // 1m 10s 982ms 441μs 221ns (default, smart units)
    Seconds,    // 70.982441221 (decimal seconds)
    Millis,     // 70982.441221 (decimal milliseconds)
    Micros,     // 70982441.221 (decimal microseconds)
    Nanos,      // 70982441221 (integer nanoseconds)
}

struct Config {
    runs: usize,
    warmup: usize,
    quiet: bool,
    median: bool,
    display: DisplayMode,
    timeout: Option<Duration>,
    compare_commands: Option<(String, String)>,
    command: Vec<String>,
}

struct TimingResult {
    times: Vec<Duration>,
    avg: Duration,
    median: Duration,
    min: Duration,
    max: Duration,
}

fn format_duration_auto(d: Duration) -> String {
    let total_ns = d.as_nanos();
    let hours   = total_ns / 3_600_000_000_000;
    let minutes = (total_ns % 3_600_000_000_000) / 60_000_000_000;
    let seconds = (total_ns % 60_000_000_000)    / 1_000_000_000;
    let millis  = (total_ns % 1_000_000_000)     / 1_000_000;
    let micros  = (total_ns % 1_000_000)         / 1_000;
    let nanos   =  total_ns % 1_000;

    let mut parts = Vec::new();
    if hours   > 0 { parts.push(format!("{}h",  hours));   }
    if minutes > 0 { parts.push(format!("{}m",  minutes)); }
    if seconds > 0 { parts.push(format!("{}s",  seconds)); }
    if millis  > 0 { parts.push(format!("{}ms", millis));  }
    if micros  > 0 { parts.push(format!("{}μs", micros));  }
    if nanos   > 0 { parts.push(format!("{}ns", nanos));   }

    if parts.is_empty() {
        "0ns".to_string()
    } else {
        parts.join(" ")
    }
}

fn format_duration(d: Duration, mode: DisplayMode) -> String {
    match mode {
        DisplayMode::Auto => format_duration_auto(d),
        DisplayMode::Seconds => format!("{:.9}", d.as_secs_f64()),
        DisplayMode::Millis => format!("{:.6}", d.as_secs_f64() * 1000.0),
        DisplayMode::Micros => format!("{:.3}", d.as_secs_f64() * 1_000_000.0),
        DisplayMode::Nanos => format!("{}", d.as_nanos()),
    }
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty duration".into());
    }
    
    // Find where the number ends
    let num_end = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());
    
    if num_end == 0 {
        return Err(format!("invalid duration: {}", s));
    }
    
    let num: f64 = s[..num_end]
        .parse()
        .map_err(|_| format!("invalid duration: {}", s))?;
    
    let suffix = &s[num_end..];
    let duration_ns = match suffix.to_ascii_lowercase().as_str() {
        "" | "s" => (num * 1_000_000_000.0) as u64,
        "ms" => (num * 1_000_000.0) as u64,
        "μs" | "us" => (num * 1_000.0) as u64,
        "ns" => num as u64,
        "m" => (num * 60.0 * 1_000_000_000.0) as u64,
        "h" => (num * 3600.0 * 1_000_000_000.0) as u64,
        _ => return Err(format!("unknown time suffix: {}", suffix)),
    };
    
    Ok(Duration::from_nanos(duration_ns))
}

fn load_profile(name: &str) -> Option<(usize, usize, bool)> {
    let exe_path = std::env::current_exe().ok()?;
    let profile_dir = exe_path.parent()?.join("timeit.d");
    let profile_path = profile_dir.join(format!("{}.profile", name));
    
    let content = std::fs::read_to_string(profile_path).ok()?;
    
    let mut runs = 1;
    let mut warmup = 0;
    let mut quiet = false;
    
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            match k.trim() {
                "runs" => runs = v.trim().parse().unwrap_or(1),
                "warmup" => warmup = v.trim().parse().unwrap_or(0),
                "quiet" => quiet = v.trim() == "true",
                _ => {}
            }
        }
    }
    
    Some((runs, warmup, quiet))
}

fn print_help() {
    let page = HelpPage::new("timeit - precise command timing utility")
        .usage("timeit [OPTIONS] <command> [args...]")
        .usage("timeit --compare \"<command1>\" \"<command2>\"")
        .section(Section::new(
            "FLAGS",
            vec![
                Row::new("-q", "--quiet",   "only show average (suppress per-run output)"),
                Row::with_value("-w", "--warmup",  "<N>",        "run N warmup iterations (5s cooldown after)"),
                Row::with_value("-r", "--runs",    "<N>",        "number of measured runs (default: 1)"),
                Row::with_value("-p", "--profile", "<NAME>",     "load profile from ~/.local/bin/timeit.d/NAME.profile"),
                Row::new("-h", "--help",    "show this help"),
                Row::new("",   "--median",  "show median instead of average"),
                Row::with_value("", "--timeout", "<DURATION>",  "kill commands after timeout (e.g., 30s, 5m, 100ms)"),
                Row::new("",   "--compare", "compare two commands (requires two quoted commands)"),
            ],
        ))
        .section(Section::with_note(
            "DISPLAY MODES",
            "Default: auto-format with smart units, e.g., 1m 10s 982ms 441μs 221ns",
            vec![
                Row::new("", "--seconds", "decimal seconds (e.g., 70.982441221)"),
                Row::new("", "--millis",  "decimal milliseconds (e.g., 70982.441221)"),
                Row::new("", "--micros",  "decimal microseconds (e.g., 70982441.221)"),
                Row::new("", "--nanos",   "integer nanoseconds (e.g., 70982441221)"),
            ],
        ))
        .section(Section::new(
            "EXAMPLES",
            vec![
                Row::new("", "timeit fasthex file.bin", ""),
                Row::new("", "timeit -r 10 --median fasthex file.bin", ""),
                Row::new("", "timeit -w 3 -r 10 --timeout 30s fasthex file.bin", ""),
                Row::new("", "timeit -q -r 100 fasthex file.bin", ""),
                Row::new("", "timeit --profile=bench fasthex file.bin", ""),
                Row::new("", "timeit --compare \"hexdump file.bin\" \"fasthex file.bin\"", ""),
                Row::new("", "timeit --timeout 5m long-running-cmd", ""),
            ],
        ))
        .section(Section::with_note(
            "TIMEOUT UNITS",
            "ns, μs/us, ms, s, m, h (e.g., 100ms, 30s, 5m, 1h)",
            vec![],
        ))
        .section(Section::with_note(
            "PROFILES",
            "Create profile files in ~/.local/bin/timeit.d/ with format:\n\n  runs=100\n  warmup=5\n  quiet=true",
            vec![],
        ));
    page.print(ColorWhen::Auto);
}

fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    
    if args.is_empty() {
        return Err("No command specified".to_string());
    }
    
    let mut runs = 1;
    let mut warmup = 0;
    let mut quiet = false;
    let mut median = false;
    let mut display = DisplayMode::Auto;
    let mut timeout = None;
    let mut compare_commands = None;
    let mut i = 0;
    
    while i < args.len() {
        let arg = &args[i];
        
        if arg == "-h" || arg == "--help" {
            print_help();
            std::process::exit(0);
        } else if arg == "-q" || arg == "--quiet" {
            quiet = true;
        } else if arg == "--median" {
            median = true;
        } else if arg == "--compare" {
            compare_commands = Some((String::new(), String::new())); // Just flag it
        } else if arg == "-w" || arg == "--warmup" {
            i += 1;
            if i >= args.len() {
                return Err("Missing value for --warmup".to_string());
            }
            warmup = args[i].parse()
                .map_err(|_| format!("Invalid warmup value: {}", args[i]))?;
        } else if arg.starts_with("--warmup=") {
            warmup = arg.strip_prefix("--warmup=").unwrap().parse()
                .map_err(|_| format!("Invalid warmup value: {}", arg))?;
        } else if arg == "-r" || arg == "--runs" {
            i += 1;
            if i >= args.len() {
                return Err("Missing value for --runs".to_string());
            }
            runs = args[i].parse()
                .map_err(|_| format!("Invalid runs value: {}", args[i]))?;
        } else if arg.starts_with("--runs=") {
            runs = arg.strip_prefix("--runs=").unwrap().parse()
                .map_err(|_| format!("Invalid runs value: {}", arg))?;
        } else if arg == "--timeout" {
            i += 1;
            if i >= args.len() {
                return Err("Missing value for --timeout".to_string());
            }
            timeout = Some(parse_duration(&args[i])?);
        } else if arg.starts_with("--timeout=") {
            timeout = Some(parse_duration(arg.strip_prefix("--timeout=").unwrap())?);
        } else if arg == "-p" || arg == "--profile" {
            i += 1;
            if i >= args.len() {
                return Err("Missing value for --profile".to_string());
            }
            let (r, w, q) = load_profile(&args[i])
                .ok_or_else(|| format!("Profile not found: {}", args[i]))?;
            runs = r;
            warmup = w;
            quiet = q;
        } else if arg.starts_with("--profile=") || arg.starts_with("-p=") {
            let name = if arg.starts_with("--profile=") {
                arg.strip_prefix("--profile=").unwrap()
            } else {
                arg.strip_prefix("-p=").unwrap()
            };
            let (r, w, q) = load_profile(name)
                .ok_or_else(|| format!("Profile not found: {}", name))?;
            runs = r;
            warmup = w;
            quiet = q;
        } else if arg == "--seconds" {
            display = DisplayMode::Seconds;
        } else if arg == "--millis" {
            display = DisplayMode::Millis;
        } else if arg == "--micros" {
            display = DisplayMode::Micros;
        } else if arg == "--nanos" {
            display = DisplayMode::Nanos;
        } else {
            // Non-flag argument found - stop parsing flags
            break;
        }
        i += 1;
    }
    
    // Handle remaining arguments
    if i < args.len() {
        if compare_commands.is_some() {
            // For --compare mode, need exactly 2 remaining arguments
            let remaining_args: Vec<String> = args[i..].to_vec();
            if remaining_args.len() != 2 {
                return Err(format!("--compare requires exactly 2 commands, got: {}", remaining_args.len()));
            }
            return Ok(Config {
                runs,
                warmup,
                quiet,
                median,
                display,
                timeout,
                compare_commands: Some((remaining_args[0].clone(), remaining_args[1].clone())),
                command: vec![],
            });
        } else {
            // Single command mode
            return Ok(Config {
                runs,
                warmup,
                quiet,
                median,
                display,
                timeout,
                compare_commands: None,
                command: args[i..].to_vec(),
            });
        }
    }
    
    if compare_commands.is_some() {
        Err("--compare requires exactly 2 commands".to_string())
    } else {
        Err("No command specified".to_string())
    }
}

fn calculate_timing_stats(times: Vec<Duration>) -> TimingResult {
    if times.is_empty() {
        return TimingResult {
            times: vec![],
            avg: Duration::ZERO,
            median: Duration::ZERO,
            min: Duration::ZERO,
            max: Duration::ZERO,
        };
    }
    
    let mut sorted_times = times.clone();
    sorted_times.sort();
    
    let min = *sorted_times.first().unwrap();
    let max = *sorted_times.last().unwrap();
    
    let avg_ns = times.iter().map(|d| d.as_nanos()).sum::<u128>() / times.len() as u128;
    let avg = Duration::from_nanos(avg_ns as u64);
    
    let median = if sorted_times.len() % 2 == 0 {
        let mid1 = sorted_times[sorted_times.len() / 2 - 1];
        let mid2 = sorted_times[sorted_times.len() / 2];
        Duration::from_nanos((mid1.as_nanos() + mid2.as_nanos()) as u64 / 2)
    } else {
        sorted_times[sorted_times.len() / 2]
    };
    
    TimingResult {
        times,
        avg,
        median,
        min,
        max,
    }
}

fn run_command_with_timeout(cmd_args: &[String], timeout: Option<Duration>, quiet: bool) -> Result<Duration, String> {
    if cmd_args.is_empty() {
        return Err("Empty command".to_string());
    }
    
    let start = std::time::Instant::now();
    
    let mut command = if cmd_args.len() == 1 {
        // Single string - always use shell for comparison mode compatibility
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&cmd_args[0]);
        cmd
    } else {
        let mut cmd = Command::new(&cmd_args[0]);
        cmd.args(&cmd_args[1..]);
        cmd
    };
    
    if quiet {
        command.stdout(std::process::Stdio::null());
    }
    
    let child = command.spawn().map_err(|e| format!("Failed to start command: {}", e))?;
    
    let result = if let Some(timeout_duration) = timeout {
        // Spawn a thread to wait for the process
        let (tx, rx) = std::sync::mpsc::channel();
        let mut child = child;
        
        thread::spawn(move || {
            let result = child.wait();
            let _ = tx.send(result);
        });
        
        // Wait for either completion or timeout
        match rx.recv_timeout(timeout_duration) {
            Ok(wait_result) => wait_result.map_err(|e| format!("Command execution failed: {}", e))?,
            Err(_) => {
                return Err(format!("Command timed out after {}", format_duration_auto(timeout_duration)));
            }
        }
    } else {
        let mut child = child;
        child.wait().map_err(|e| format!("Command execution failed: {}", e))?
    };
    
    let elapsed = start.elapsed();
    
    if !result.success() {
        return Err(format!("Command failed with exit code: {}", result.code().unwrap_or(-1)));
    }
    
    Ok(elapsed)
}

fn time_command(config: &Config, cmd_args: &[String], cmd_name: &str) -> Result<TimingResult, String> {
    if !config.quiet {
        eprintln!();
        if !cmd_name.is_empty() {
            eprintln!("Timing: {}", cmd_name);
        }
    }
    
    // Warmup phase
    if config.warmup > 0 && !config.quiet {
        eprintln!("Warming up... ({} runs)", config.warmup);
    }
    
    for _ in 0..config.warmup {
        run_command_with_timeout(cmd_args, config.timeout, true)?; // Always quiet for warmup
    }
    
    // Wait 5 seconds after warmup
    if config.warmup > 0 {
        if !config.quiet {
            eprintln!("Cooldown (5s)...");
        }
        thread::sleep(Duration::from_secs(5));
    }
    
    // Actual measured runs
    let mut times = Vec::new();
    
    for i in 1..=config.runs {
        let elapsed = run_command_with_timeout(cmd_args, config.timeout, config.quiet)?;
        
        if !config.quiet {
            eprintln!("Run {}/{}: {}", i, config.runs, format_duration(elapsed, config.display));
        }
        
        times.push(elapsed);
    }
    
    Ok(calculate_timing_stats(times))
}



fn main() {
    let config = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Try 'timeit --help' for usage information.");
            std::process::exit(1);
        }
    };
    
    if let Some((cmd1_str, cmd2_str)) = &config.compare_commands {
        // Comparison mode
        eprintln!();
        eprintln!("=== COMMAND COMPARISON ===");
        
        let result1 = match time_command(&config, &[cmd1_str.clone()], &format!("Command A: {}", cmd1_str)) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Command A failed: {}", e);
                std::process::exit(1);
            }
        };
        
        let result2 = match time_command(&config, &[cmd2_str.clone()], &format!("Command B: {}", cmd2_str)) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Command B failed: {}", e);
                std::process::exit(1);
            }
        };
        
        eprintln!();
        eprintln!("=== RESULTS ===");
        
        let stat1 = if config.median { result1.median } else { result1.avg };
        let stat2 = if config.median { result2.median } else { result2.avg };
        let stat_name = if config.median { "Median" } else { "Average" };
        
        eprintln!("Command A: {}", cmd1_str);
        eprintln!("{}: {}", stat_name, format_duration(stat1, config.display));
        eprintln!();
        eprintln!("Command B: {}", cmd2_str);
        eprintln!("{}: {}", stat_name, format_duration(stat2, config.display));
        eprintln!();
        
        // Calculate speedup
        let (faster_cmd, faster_time, slower_time) = if stat1 < stat2 {
            ("A", stat1, stat2)
        } else {
            ("B", stat2, stat1)
        };
        
        let speedup = slower_time.as_secs_f64() / faster_time.as_secs_f64();
        eprintln!("Command {} is {:.1}× faster", faster_cmd, speedup);
        
    } else {
        // Single command mode
        if config.command.is_empty() {
            eprintln!("Error: No command specified");
            std::process::exit(1);
        }
        
        let result = match time_command(&config, &config.command, "") {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Command failed: {}", e);
                std::process::exit(1);
            }
        };
        
        let final_stat = if config.median { result.median } else { result.avg };
        let stat_name = if config.median { "Median" } else { "Average" };
        
        if config.quiet {
            println!("{}", format_duration(final_stat, config.display));
        } else {
            eprintln!("==============================");
            eprintln!("{} ({}): {}", stat_name, config.runs, format_duration(final_stat, config.display));
        }
    }
}

