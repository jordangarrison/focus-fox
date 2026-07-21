use std::process::Command;

/// Fire a desktop notification via notify-send. Best-effort: missing binary
/// or a failed call never interrupts the timer.
pub fn send(summary: &str, body: &str) {
    let _ = Command::new("notify-send")
        .arg("--app-name=Focus Fox")
        .arg(summary)
        .arg(body)
        .spawn();
}
