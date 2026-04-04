use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    // Read one line from stdin (the plan message)
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_err() {
        eprintln!("Failed to read from stdin");
        std::process::exit(1);
    }

    // Stub: echo back a validation failure for now
    let response = serde_json::json!({
        "type": "validation",
        "v": 1,
        "payload": {
            "ok": false,
            "errors": [{"code": "NOT_IMPLEMENTED", "message": "Stub runtime", "field": null}],
            "warnings": [],
            "effectiveState": null
        }
    });

    writeln!(stdout, "{}", response).unwrap();
    stdout.flush().unwrap();
}
