use crate::contract::{BlockedConnection, ObservedConnection};

/// Return currently observed network connections.
/// This is a stub — real implementation would hook into OS network APIs.
pub fn observe_connections() -> Vec<ObservedConnection> {
    vec![]
}

/// Compute which observed connections would have been blocked under the given allowlist.
///
/// The allowlist contains entries in "host:port" format.
/// A connection is "would-have-blocked" if it is not matched by any allowlist entry.
pub fn compute_would_have_blocked(
    observed: &[ObservedConnection],
    allowlist: &Option<Vec<String>>,
) -> Vec<BlockedConnection> {
    let Some(list) = allowlist else {
        // No allowlist configured — nothing would have been blocked.
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
