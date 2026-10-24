use regex::Regex;
use std::fs;

pub fn scan_logs(paths: &[String]) -> Vec<String> {
    let mut results = Vec::new();
    let re = Regex::new(r"(?i)error|fail|critical|panic|unauthorized|denied").unwrap();

    for path in paths {
        match fs::read_to_string(path) {
            Ok(content) => {
                for line in content.lines() {
                    if re.is_match(line) {
                        results.push(format!("{}: {}", path, line));
                    }
                }
            }
            Err(_) => {
                results.push(format!("Could not open log file: {}", path));
            }
        }
    }
    results
}
