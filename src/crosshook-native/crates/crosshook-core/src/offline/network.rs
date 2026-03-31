use std::net::{SocketAddr, TcpStream};
use std::str::FromStr;
use std::time::Duration;

const PROBE_ENDPOINTS: &[&str] = &["8.8.8.8:53", "1.1.1.1:53", "208.67.222.222:53"];

/// Quick connectivity probe — tries multiple public DNS resolvers and returns
/// `true` on the first successful TCP connect.
pub fn is_network_available() -> bool {
    PROBE_ENDPOINTS.iter().any(|endpoint| {
        SocketAddr::from_str(endpoint)
            .map(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok())
            .unwrap_or(false)
    })
}
