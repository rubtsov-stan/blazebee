//! Connection lifecycle: client construction, the event-loop driver task,
//! reconnection policy and graceful shutdown.

use std::{fmt, fs, time::Duration};

use rumqttc::{
    AsyncClient, ConnectReturnCode, ConnectionError, Event, EventLoop, LastWill, MqttOptions,
    Outgoing, Packet, QoS, StateError, TlsConfiguration, Transport as RumqttTransport,
};
use tokio::{
    sync::{oneshot, watch},
    task::JoinHandle,
};
use tracing::{debug, error, info, warn};

use crate::{
    backoff::Backoff,
    config::Config,
    error::TransferError,
    publisher::{join_topic, Publisher},
};

/// Payload published on `{base_topic}/status`.
pub const STATUS_ONLINE: &str = "online";
pub const STATUS_OFFLINE: &str = "offline";

/// Time allowed for the broker to receive the final `offline` status and the
/// DISCONNECT packet during shutdown.
const DISCONNECT_GRACE: Duration = Duration::from_secs(3);

/// Observable state of the broker connection.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Initial state until the first CONNACK.
    Connecting,
    Connected,
    /// The connection was lost or refused; the reason is the root-cause message.
    Disconnected(String),
    /// Waiting for the given number of seconds before the next attempt.
    Reconnecting(f64),
    /// An unrecoverable error; the driver has stopped.
    FatalExit(String),
}

impl ConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Connected)
    }

    pub fn is_fatal(&self) -> bool {
        matches!(self, ConnectionState::FatalExit(_))
    }
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionState::Connecting => f.write_str("connecting"),
            ConnectionState::Connected => f.write_str("connected"),
            ConnectionState::Disconnected(reason) => write!(f, "disconnected ({reason})"),
            ConnectionState::Reconnecting(secs) => write!(f, "reconnecting in {secs:.1} s"),
            ConnectionState::FatalExit(reason) => write!(f, "fatal ({reason})"),
        }
    }
}

/// Handle to a running connection. Dropping it without calling
/// [`Transport::shutdown`] aborts the driver and closes the socket without a
/// DISCONNECT packet, which makes the broker publish the last will.
pub struct Transport {
    client: AsyncClient,
    state: watch::Receiver<ConnectionState>,
    shutdown: Option<oneshot::Sender<()>>,
    driver: JoinHandle<Result<(), TransferError>>,
    config: Config,
}

impl Transport {
    /// Validates `config`, builds the client and spawns the driver on the
    /// current Tokio runtime. Returns before the first connection attempt
    /// completes; watch [`Transport::state`] for progress.
    pub fn connect(config: &Config) -> Result<Transport, TransferError> {
        config.validate()?;
        let options = build_options(config)?;
        let capacity = usize::from(config.request_channel_capacity);
        let (client, eventloop) = AsyncClient::new(options, capacity);
        let (state_tx, state_rx) = watch::channel(ConnectionState::Connecting);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let driver = Driver {
            eventloop,
            client: client.clone(),
            state: state_tx,
            backoff: Backoff::default(),
            status_topic: join_topic(&config.base_topic, "status"),
        };
        info!(
            host = %config.host,
            port = config.port,
            client_id = %config.client_id,
            tls = config.tls.is_enabled(),
            base_topic = %config.base_topic,
            "starting MQTT connection"
        );
        let handle = tokio::spawn(driver.run(shutdown_rx));
        Ok(Transport {
            client,
            state: state_rx,
            shutdown: Some(shutdown_tx),
            driver: handle,
            config: config.clone(),
        })
    }

    /// Receiver for connection state changes. The current value is available
    /// immediately through `borrow()`.
    pub fn state(&self) -> watch::Receiver<ConnectionState> {
        self.state.clone()
    }

    /// Creates a publisher that encodes with this transport's serialization
    /// settings and applies its base topic.
    pub fn publisher(&self) -> Publisher {
        Publisher::new(self.client.clone(), &self.config)
    }

    /// Resolves when the driver stops on its own, which only happens on a
    /// fatal error. The error is also visible as `ConnectionState::FatalExit`.
    pub async fn closed(&mut self) -> Result<(), TransferError> {
        match (&mut self.driver).await {
            Ok(result) => result,
            Err(join) => Err(TransferError::Closed)
                .inspect_err(|_| error!(%join, "connection driver panicked")),
        }
    }

    /// Publishes `offline` on the status topic, sends DISCONNECT and waits for
    /// the driver to finish, for at most `timeout`.
    pub async fn shutdown(mut self, timeout: Duration) -> Result<(), TransferError> {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        match tokio::time::timeout(timeout, &mut self.driver).await {
            Ok(Ok(result)) => result,
            Ok(Err(join)) => {
                error!(%join, "connection driver panicked during shutdown");
                Err(TransferError::Closed)
            }
            Err(_) => {
                warn!("connection driver did not stop within {timeout:?}; aborting");
                self.driver.abort();
                Ok(())
            }
        }
    }
}

fn build_options(config: &Config) -> Result<MqttOptions, TransferError> {
    let mut options = MqttOptions::new(config.client_id.clone(), config.host.trim(), config.port);
    options
        .set_keep_alive(Duration::from_secs(u64::from(config.keep_alive)))
        .set_clean_session(config.clean_session)
        .set_inflight(config.max_inflight)
        .set_max_packet_size(
            config.max_packet_size as usize,
            config.max_packet_size as usize,
        )
        .set_last_will(LastWill::new(
            join_topic(&config.base_topic, "status"),
            STATUS_OFFLINE,
            QoS::AtLeastOnce,
            true,
        ));
    if let Some(username) = &config.username {
        let password = config
            .password
            .as_ref()
            .map(|p| p.expose())
            .unwrap_or_default();
        options.set_credentials(username.clone(), password);
    }
    options.set_transport(build_transport(config)?);
    Ok(options)
}

fn build_transport(config: &Config) -> Result<RumqttTransport, TransferError> {
    let tls = &config.tls;
    if !tls.is_enabled() {
        return Ok(RumqttTransport::Tcp);
    }
    let Some(ca_path) = &tls.ca_cert_path else {
        return Ok(RumqttTransport::tls_with_default_config());
    };
    let read = |path: &std::path::Path| {
        fs::read(path).map_err(|source| TransferError::ReadFile {
            path: path.to_path_buf(),
            source,
        })
    };
    let ca = read(ca_path)?;
    let client_auth = match (&tls.client_cert_path, &tls.client_key_path) {
        (Some(cert), Some(key)) => Some((read(cert)?, read(key)?)),
        _ => None,
    };
    Ok(RumqttTransport::Tls(TlsConfiguration::Simple {
        ca,
        alpn: None,
        client_auth,
    }))
}

struct Driver {
    eventloop: EventLoop,
    client: AsyncClient,
    state: watch::Sender<ConnectionState>,
    backoff: Backoff,
    status_topic: String,
}

impl Driver {
    async fn run(mut self, mut shutdown: oneshot::Receiver<()>) -> Result<(), TransferError> {
        loop {
            let event = tokio::select! {
                biased;
                _ = &mut shutdown => return self.disconnect().await,
                event = self.eventloop.poll() => event,
            };
            match event {
                Ok(Event::Incoming(Packet::ConnAck(ack))) => {
                    if ack.code == ConnectReturnCode::Success {
                        info!(session_present = ack.session_present, "connected to broker");
                        self.backoff.reset();
                        self.set_state(ConnectionState::Connected);
                        if let Err(e) = self.client.try_publish(
                            &self.status_topic,
                            QoS::AtLeastOnce,
                            true,
                            STATUS_ONLINE,
                        ) {
                            warn!(error = %e, "could not queue online status");
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    let reason = root_cause(&e);
                    if is_fatal(&e) {
                        error!(error = %reason, "unrecoverable connection error");
                        self.set_state(ConnectionState::FatalExit(reason));
                        return Err(e.into());
                    }
                    let delay = self.backoff.next_delay();
                    warn!(error = %reason, retry_in_s = format_args!("{:.1}", delay.as_secs_f64()), "connection lost");
                    self.set_state(ConnectionState::Disconnected(reason));
                    self.set_state(ConnectionState::Reconnecting(delay.as_secs_f64()));
                    tokio::select! {
                        biased;
                        _ = &mut shutdown => return Ok(()),
                        _ = tokio::time::sleep(delay) => {}
                    }
                }
            }
        }
    }

    /// Best-effort clean disconnect: queue the final status, queue DISCONNECT
    /// and keep polling until the DISCONNECT has been written or the grace
    /// period ends. When not connected there is nothing to flush.
    async fn disconnect(mut self) -> Result<(), TransferError> {
        if self.state.borrow().is_connected() {
            let _ =
                self.client
                    .try_publish(&self.status_topic, QoS::AtLeastOnce, true, STATUS_OFFLINE);
            let _ = self.client.try_disconnect();
            let deadline = tokio::time::Instant::now() + DISCONNECT_GRACE;
            loop {
                match tokio::time::timeout_at(deadline, self.eventloop.poll()).await {
                    Ok(Ok(Event::Outgoing(Outgoing::Disconnect))) => {
                        debug!("DISCONNECT sent");
                        break;
                    }
                    Ok(Ok(_)) => continue,
                    Ok(Err(e)) => {
                        debug!(error = %root_cause(&e), "connection closed during shutdown");
                        break;
                    }
                    Err(_) => {
                        warn!("broker did not accept DISCONNECT within {DISCONNECT_GRACE:?}");
                        break;
                    }
                }
            }
        }
        self.set_state(ConnectionState::Disconnected("shutdown".into()));
        info!("MQTT connection closed");
        Ok(())
    }

    fn set_state(&self, next: ConnectionState) {
        self.state.send_if_modified(|current| {
            if *current == next {
                false
            } else {
                *current = next;
                true
            }
        });
    }
}

/// Errors that a new connection cannot fix: bad credentials, rejected
/// protocol or client id, TLS configuration, local resource problems, or the
/// client handles being gone. Everything else is retried.
pub(crate) fn is_fatal(err: &ConnectionError) -> bool {
    match err {
        ConnectionError::Tls(_) => true,
        ConnectionError::NotConnAck(_) => true,
        ConnectionError::RequestsDone => true,
        ConnectionError::ConnectionRefused(code) => matches!(
            code,
            ConnectReturnCode::RefusedProtocolVersion
                | ConnectReturnCode::BadClientId
                | ConnectReturnCode::BadUserNamePassword
                | ConnectReturnCode::NotAuthorized
        ),
        ConnectionError::Io(e) => matches!(
            e.kind(),
            std::io::ErrorKind::AddrInUse
                | std::io::ErrorKind::PermissionDenied
                | std::io::ErrorKind::InvalidInput
        ),
        ConnectionError::MqttState(StateError::InvalidState | StateError::EmptySubscription) => {
            true
        }
        _ => false,
    }
}

/// Innermost error message of a chain, without surrounding quotes.
pub(crate) fn root_cause(err: &dyn std::error::Error) -> String {
    let mut current = err;
    while let Some(source) = current.source() {
        current = source;
    }
    current.to_string().trim_matches('"').to_string()
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    #[test]
    fn classifies_errors() {
        let refused =
            ConnectionError::Io(io::Error::new(io::ErrorKind::ConnectionRefused, "refused"));
        assert!(!is_fatal(&refused));
        let timeout = ConnectionError::NetworkTimeout;
        assert!(!is_fatal(&timeout));
        let ping = ConnectionError::MqttState(StateError::AwaitPingResp);
        assert!(!is_fatal(&ping));
        let busy = ConnectionError::ConnectionRefused(ConnectReturnCode::ServiceUnavailable);
        assert!(!is_fatal(&busy));

        let auth = ConnectionError::ConnectionRefused(ConnectReturnCode::BadUserNamePassword);
        assert!(is_fatal(&auth));
        let perm = ConnectionError::Io(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));
        assert!(is_fatal(&perm));
        assert!(is_fatal(&ConnectionError::RequestsDone));
    }

    #[test]
    fn root_cause_unwraps_chain() {
        let inner = io::Error::other("\"deep\"");
        let err = ConnectionError::Io(inner);
        assert_eq!(root_cause(&err), "deep");
    }

    #[test]
    fn state_display() {
        assert_eq!(
            ConnectionState::Reconnecting(2.0).to_string(),
            "reconnecting in 2.0 s"
        );
        assert_eq!(
            ConnectionState::Disconnected("x".into()).to_string(),
            "disconnected (x)"
        );
        assert!(ConnectionState::FatalExit("x".into()).is_fatal());
        assert!(ConnectionState::Connected.is_connected());
    }

    fn config() -> Config {
        Config {
            client_id: "test".into(),
            host: "127.0.0.1".into(),
            port: 1,
            ..Config::default()
        }
    }

    #[test]
    fn builds_options_with_credentials_and_will() {
        let mut c = config();
        c.username = Some("u".into());
        c.password = Some(crate::config::Secret::new("p"));
        let options = build_options(&c).unwrap();
        let login = options.credentials().unwrap();
        assert_eq!(
            (login.username.as_str(), login.password.as_str()),
            ("u", "p")
        );
        let will = options.last_will().unwrap();
        assert_eq!(will.topic, "blazebee_mqtt_v3/status");
        assert_eq!(&will.message[..], STATUS_OFFLINE.as_bytes());
        assert!(will.retain);
        assert_eq!(options.keep_alive(), Duration::from_secs(60));
    }

    #[test]
    fn tls_without_ca_uses_system_roots_and_missing_ca_is_reported() {
        let mut c = config();
        c.tls.enabled = true;
        assert!(matches!(
            build_transport(&c).unwrap(),
            RumqttTransport::Tls(_)
        ));
        c.tls.ca_cert_path = Some("/nonexistent/ca.pem".into());
        assert!(matches!(
            build_transport(&c),
            Err(TransferError::ReadFile { .. })
        ));
    }

    #[tokio::test]
    async fn unreachable_broker_reconnects_and_shuts_down_cleanly() {
        let transport = Transport::connect(&config()).unwrap();
        let mut state = transport.state();
        // Port 1 is closed: expect a Disconnected/Reconnecting transition, never Fatal.
        let seen = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                state.changed().await.unwrap();
                let s = state.borrow().clone();
                if !matches!(s, ConnectionState::Connecting) {
                    break s;
                }
            }
        })
        .await
        .expect("state change");
        assert!(
            matches!(
                seen,
                ConnectionState::Disconnected(_) | ConnectionState::Reconnecting(_)
            ),
            "{seen}"
        );
        let started = std::time::Instant::now();
        transport.shutdown(Duration::from_secs(2)).await.unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
