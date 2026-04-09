use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::contract::{BlockedConnection, ObservedConnection};

/// Background network observer that polls /proc/net/tcp for outbound connections.
///
/// On Linux: polls at ~500ms intervals, deduplicates, emits network events.
/// On non-Linux: no-op (returns empty results immediately).
pub struct NetworkObserver {
    #[cfg(target_os = "linux")]
    handle: Option<std::thread::JoinHandle<Vec<ObservedConnection>>>,
    stop_flag: Arc<AtomicBool>,
}

impl NetworkObserver {
    /// Start the observer. On Linux, spawns a polling thread.
    /// On non-Linux, returns a no-op observer.
    #[cfg(target_os = "linux")]
    pub fn start(seq: Arc<std::sync::atomic::AtomicU64>) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop_flag);
        let handle = std::thread::spawn(move || poll_loop(flag, seq));
        NetworkObserver {
            handle: Some(handle),
            stop_flag,
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn start(_seq: Arc<std::sync::atomic::AtomicU64>) -> Self {
        NetworkObserver {
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Stop the observer and return all observed connections.
    pub fn stop(self) -> Vec<ObservedConnection> {
        self.stop_flag.store(true, Ordering::Relaxed);
        #[cfg(target_os = "linux")]
        {
            match self.handle {
                Some(h) => h.join().unwrap_or_default(),
                None => vec![],
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            vec![]
        }
    }
}

/// The polling loop (Linux only).
#[cfg(target_os = "linux")]
fn poll_loop(
    stop_flag: Arc<AtomicBool>,
    seq: Arc<std::sync::atomic::AtomicU64>,
) -> Vec<ObservedConnection> {
    use std::collections::HashSet;
    use std::sync::atomic::Ordering as Ord;
    use std::time::Duration;

    let mut seen: HashSet<(String, u16)> = HashSet::new();
    let mut results: Vec<ObservedConnection> = Vec::new();

    loop {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        if let Ok(connections) = parse_proc_net_tcp("/proc/net/tcp") {
            for conn in connections {
                if seen.insert((conn.host.clone(), conn.port)) {
                    let s = seq.fetch_add(1, Ord::SeqCst);
                    let event = serde_json::json!({
                        "type": "network",
                        "sequence": s,
                        "ts": crate::timestamps::now_iso8601(),
                        "payload": {
                            "direction": "outbound",
                            "host": &conn.host,
                            "port": conn.port,
                            "protocol": "tcp"
                        }
                    });
                    println!("{}", event);
                    results.push(conn);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    results
}

/// Parse /proc/net/tcp and return outbound established connections.
#[cfg(target_os = "linux")]
fn parse_proc_net_tcp(path: &str) -> std::io::Result<Vec<ObservedConnection>> {
    use std::io::{BufRead, BufReader};
    use std::fs::File;

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut connections = Vec::new();

    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        if i == 0 { continue; }

        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 4 { continue; }

        let state = fields[3];
        if state != "01" { continue; }

        let rem_addr = fields[2];
        let parts: Vec<&str> = rem_addr.split(':').collect();
        if parts.len() != 2 { continue; }

        let ip_hex = parts[0];
        let port_hex = parts[1];

        let ip_u32 = match u32::from_str_radix(ip_hex, 16) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let a = (ip_u32 & 0xFF) as u8;
        let b = ((ip_u32 >> 8) & 0xFF) as u8;
        let c = ((ip_u32 >> 16) & 0xFF) as u8;
        let d = ((ip_u32 >> 24) & 0xFF) as u8;

        if a == 127 || (a == 0 && b == 0 && c == 0 && d == 0) { continue; }

        let host = format!("{a}.{b}.{c}.{d}");
        let port = match u16::from_str_radix(port_hex, 16) {
            Ok(v) => v,
            Err(_) => continue,
        };

        connections.push(ObservedConnection {
            direction: "outbound".to_string(),
            host,
            port,
            protocol: Some("tcp".to_string()),
        });
    }

    Ok(connections)
}

/// Compute which observed connections would have been blocked under the given allowlist.
pub fn compute_would_have_blocked(
    observed: &[ObservedConnection],
    allowlist: &Option<Vec<String>>,
) -> Vec<BlockedConnection> {
    let Some(list) = allowlist else {
        return vec![];
    };

    observed
        .iter()
        .filter(|conn| {
            let entry = format!("{}:{}", conn.host, conn.port);
            !list.iter().any(|allowed| allowed == &entry)
        })
        .map(|conn| BlockedConnection {
            direction: conn.direction.clone(),
            host: conn.host.clone(),
            port: conn.port,
            protocol: conn.protocol.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_would_have_blocked_with_no_allowlist() {
        let observed = vec![ObservedConnection {
            direction: "outbound".to_string(),
            host: "1.2.3.4".to_string(),
            port: 443,
            protocol: Some("tcp".to_string()),
        }];
        let blocked = compute_would_have_blocked(&observed, &None);
        assert!(blocked.is_empty());
    }

    #[test]
    fn compute_would_have_blocked_with_matching_allowlist() {
        let observed = vec![ObservedConnection {
            direction: "outbound".to_string(),
            host: "1.2.3.4".to_string(),
            port: 443,
            protocol: Some("tcp".to_string()),
        }];
        let allowlist = Some(vec!["1.2.3.4:443".to_string()]);
        let blocked = compute_would_have_blocked(&observed, &allowlist);
        assert!(blocked.is_empty());
    }

    #[test]
    fn compute_would_have_blocked_with_non_matching_allowlist() {
        let observed = vec![ObservedConnection {
            direction: "outbound".to_string(),
            host: "1.2.3.4".to_string(),
            port: 443,
            protocol: Some("tcp".to_string()),
        }];
        let allowlist = Some(vec!["5.6.7.8:443".to_string()]);
        let blocked = compute_would_have_blocked(&observed, &allowlist);
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].host, "1.2.3.4");
        assert_eq!(blocked[0].port, 443);
    }

    #[test]
    fn network_observer_noop_on_stop() {
        let seq = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let observer = NetworkObserver::start(seq);
        let connections = observer.stop();
        let _ = connections;
    }
}
