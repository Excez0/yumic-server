use anyhow::{bail, Context, Result};
use std::net::SocketAddr;
use std::sync::mpsc::{Receiver as StdReceiver, Sender as StdSender};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

const CMD_HELLO: u8 = 0x65;
const CMD_SET_MEDIA: u8 = 0x66;
const CMD_START: u8 = 0x67;
const CMD_HEARTBEAT: u8 = 0x69;
const HEADER_TOTAL: usize = 11;

struct Response {
    cmd: u8,
    payload: Vec<u8>,
}

async fn send_cmd(tcp: &mut TcpStream, cmd: u8, payload: &[u8]) -> Result<()> {
    let len = payload.len() as u32;
    let mut header = [0u8; 5];
    header[0] = cmd;
    header[1..5].copy_from_slice(&len.to_be_bytes());
    tcp.write_all(&header).await.context("send cmd header")?;
    if !payload.is_empty() {
        tcp.write_all(payload).await.context("send cmd payload")?;
    }
    tcp.flush().await.context("Failed to flush TCP stream")?;
    debug!("Sent cmd=0x{:02X}, {} bytes", cmd, payload.len());
    Ok(())
}

async fn read_response(tcp: &mut TcpStream) -> Result<Response> {
    let mut cmd_buf = [0u8; 1];
    tcp.read_exact(&mut cmd_buf).await.context("read cmd")?;
    let mut len_buf = [0u8; 4];
    tcp.read_exact(&mut len_buf).await.context("read len")?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    if len > 0 {
        tcp.read_exact(&mut payload).await.context("read payload")?;
    }
    debug!("Response cmd=0x{:02X}, {} bytes", cmd_buf[0], len);
    Ok(Response { cmd: cmd_buf[0], payload })
}

/// Events from backend -> UI
#[derive(Debug, Clone)]
pub enum BackendEvent {
    Connected,
    Disconnected,
    Error(String),
    AudioLevel(f64),
    Stats { packets: u64, errors: u64, bytes: u64 },
}

/// Commands from UI -> backend
#[derive(Debug, Clone)]
pub enum BackendCommand {
    Connect { ip: String, control_port: u16, media_port: u16 },
    Disconnect,
}

struct ConnectedSession {
    tcp_write: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    udp: UdpSocket,
    recv_buf: Vec<u8>,
}

/// Spawn the backend on a dedicated thread with its own tokio runtime.
pub fn spawn_backend(
    event_tx: StdSender<BackendEvent>,
    cmd_rx: StdReceiver<BackendCommand>,
    pipe_path: &str,
    source_name: &str,
) {
    let pipe_path = pipe_path.to_string();
    let source_name = source_name.to_string();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
        rt.block_on(async move {
            // Bridge std::sync::mpsc -> tokio::sync::mpsc
            let (tokio_cmd_tx, mut tokio_cmd_rx) = tokio::sync::mpsc::unbounded_channel::<BackendCommand>();
            let tokio_cmd_tx_inner = tokio_cmd_tx.clone();
            std::thread::spawn(move || {
                while let Ok(cmd) = cmd_rx.recv() {
                    if tokio_cmd_tx_inner.send(cmd).is_err() {
                        break;
                    }
                }
            });

            run_backend(&pipe_path, &source_name, event_tx, &mut tokio_cmd_rx, tokio_cmd_tx).await;
        });
    });
}

async fn run_backend(
    pipe_path: &str,
    source_name: &str,
    event_tx: StdSender<BackendEvent>,
    cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<BackendCommand>,
    cmd_tx: tokio::sync::mpsc::UnboundedSender<BackendCommand>,
) {
    let mut opus_decoder = match crate::audio::OpusDecoder::new() {
        Ok(d) => d,
        Err(e) => {
            let _ = event_tx.send(BackendEvent::Error(format!("Opus init failed: {}", e)));
            return;
        }
    };

    let mut pipe_writer = crate::audio::PipeWriter::new(pipe_path);
    let mut pa_modules: Option<crate::audio::PulseAudioModules> = None;

    let mut connection: Option<ConnectedSession> = None;
    let mut heartbeat_handle: Option<tokio::task::JoinHandle<()>> = None;
    let mut drain_handle: Option<tokio::task::JoinHandle<()>> = None;

    let mut total_packets: u64 = 0;
    let mut total_errors: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut level_accumulator: f64 = 0.0;
    let mut level_count: u32 = 0;

    let mut last_packet_time = tokio::time::Instant::now();
    let mut reconnect_info: Option<(String, u16, u16)> = None;

    info!("Backend thread started");

    let mut stats_interval = tokio::time::interval(Duration::from_secs(3));
    let mut level_interval = tokio::time::interval(Duration::from_millis(100));
    let mut watchdog_interval = tokio::time::interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(BackendCommand::Connect { ip, control_port, media_port }) => {
                        // Disconnect existing
                        if let Some(old) = connection.take() {
                            let _ = old.tcp_write.lock().await.shutdown().await;
                            if let Some(h) = heartbeat_handle.take() { h.abort(); }
                            if let Some(h) = drain_handle.take() { h.abort(); }
                        }
                        if let Some(mut pm) = pa_modules.take() {
                            let _ = pm.cleanup();
                        }

                        info!("Connecting to {}:{}...", ip, control_port);

                        let mut pm = crate::audio::PulseAudioModules::new("yumic_sink", source_name);
                        if let Err(e) = pm.setup() {
                            let _ = event_tx.send(BackendEvent::Error(format!("PA setup: {}", e)));
                            continue;
                        }
                        pa_modules = Some(pm);

                        if let Err(e) = pipe_writer.open() {
                            let _ = event_tx.send(BackendEvent::Error(format!("Pipe: {}", e)));
                            continue;
                        }

                         match connect_to_phone(&ip, control_port, media_port).await {
                            Ok((session, drain, hb)) => {
                                connection = Some(session);
                                heartbeat_handle = Some(hb);
                                drain_handle = Some(drain);
                                total_packets = 0;
                                total_errors = 0;
                                total_bytes = 0;
                                last_packet_time = tokio::time::Instant::now();
                                reconnect_info = Some((ip, control_port, media_port));
                                let _ = event_tx.send(BackendEvent::Connected);
                                info!("Connected!");
                            }
                            Err(e) => {
                                let _ = event_tx.send(BackendEvent::Error(format!("Connect: {}", e)));
                            }
                        }
                    }
                    Some(BackendCommand::Disconnect) => {
                        if let Some(session) = connection.take() {
                            let _ = session.tcp_write.lock().await.shutdown().await;
                        }
                        if let Some(h) = heartbeat_handle.take() { h.abort(); }
                        if let Some(h) = drain_handle.take() { h.abort(); }
                        if let Some(mut pm) = pa_modules.take() {
                            let _ = pm.cleanup();
                        }
                        pipe_writer.close();
                        let _ = event_tx.send(BackendEvent::Disconnected);
                        info!("Disconnected");
                    }
                    None => {
                        info!("Command channel closed, shutting down backend");
                        break;
                    }
                }
            }

            result = async {
                if let Some(ref mut sess) = connection {
                    sess.udp.recv(&mut sess.recv_buf).await
                } else {
                    std::future::pending::<std::io::Result<usize>>().await
                }
            }, if connection.is_some() => {
                match result {
                    Ok(n) if n > HEADER_TOTAL => {
                        last_packet_time = tokio::time::Instant::now();
                        total_packets += 1;
                        let audio_data = &connection.as_ref().unwrap().recv_buf[HEADER_TOTAL..n];
                        total_bytes += audio_data.len() as u64;

                        match opus_decoder.decode(audio_data) {
                            Ok(pcm_data) => {
                                let samples: Vec<f32> = pcm_data.chunks_exact(2)
                                    .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
                                    .collect();
                                if !samples.is_empty() {
                                    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
                                    level_accumulator += rms as f64;
                                    level_count += 1;
                                }

                                if let Err(e) = pipe_writer.write(&pcm_data) {
                                    warn!("Pipe write: {:?}", e);
                                }
                            }
                            Err(_) => {
                                total_errors += 1;
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!("UDP recv: {:?}", e);
                    }
                }
            }

            _ = watchdog_interval.tick(), if connection.is_some() => {
                if last_packet_time.elapsed() > Duration::from_secs(3) {
                    warn!("No packets for 3s. Reconnecting...");
                    
                    // FIRST send Disconnected so UI updates immediately
                    let _ = event_tx.send(BackendEvent::Disconnected);
                    
                    // Then error message for reconnect info
                    let _ = event_tx.send(BackendEvent::Error("Connection lost, reconnecting...".into()));
                    
                    // Disconnect gracefully
                    if let Some(session) = connection.take() {
                        let _ = session.tcp_write.lock().await.shutdown().await;
                    }
                    if let Some(h) = heartbeat_handle.take() { h.abort(); }
                    if let Some(h) = drain_handle.take() { h.abort(); }
                    
                    // Reconnect logic
                    if let Some((ip, cp, mp)) = reconnect_info.clone() {
                        let _ = cmd_tx.send(BackendCommand::Connect { ip, control_port: cp, media_port: mp });
                    }
                }
            }
            
            _ = stats_interval.tick(), if connection.is_some() => {
                let _ = event_tx.send(BackendEvent::Stats {
                    packets: total_packets,
                    errors: total_errors,
                    bytes: total_bytes,
                });
            }

            _ = level_interval.tick(), if connection.is_some() => {
                let level = if level_count > 0 {
                    (level_accumulator / level_count as f64).min(1.0)
                } else {
                    0.0
                };
                let _ = event_tx.send(BackendEvent::AudioLevel(level));
                level_accumulator = 0.0;
                level_count = 0;
            }

            else => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

async fn connect_to_phone(
    ip: &str,
    control_port: u16,
    media_port: u16,
) -> Result<(ConnectedSession, tokio::task::JoinHandle<()>, tokio::task::JoinHandle<()>)> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV6,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    socket.set_reuse_address(true)?;
    socket.set_only_v6(false)?;
    let sock_addr: SocketAddr = format!("[::]:{}", media_port).parse()?;
    socket.bind(&socket2::SockAddr::from(sock_addr))?;
    socket.set_nonblocking(true)?;
    let udp_std: std::net::UdpSocket = socket.into();
    let udp = tokio::net::UdpSocket::from_std(udp_std)?;

    let target = format!("{}:{}", ip, control_port);
    let mut tcp = TcpStream::connect(&target).await
        .with_context(|| format!("TCP connect to {} failed", target))?;

    let hello: [u8; 6] = [0x04, 0x04, 0x06, 0x02, 0x00, 0x00];
    send_cmd(&mut tcp, CMD_HELLO, &hello).await?;
    let resp = read_response(&mut tcp).await?;
    if resp.cmd != CMD_HELLO { bail!("Expected HELLO echo"); }

    let mut media_payload: [u8; 6] = [0x02, 0x02, 0x00, 0x00, 0, 0];
    media_payload[4] = (media_port >> 8) as u8;
    media_payload[5] = (media_port & 0xFF) as u8;
    send_cmd(&mut tcp, CMD_SET_MEDIA, &media_payload).await?;
    let resp = read_response(&mut tcp).await?;
    if resp.cmd != CMD_SET_MEDIA || resp.payload.first() != Some(&0x00) {
        bail!("SET_MEDIA failed");
    }

    send_cmd(&mut tcp, CMD_START, &[]).await?;
    let resp = read_response(&mut tcp).await?;
    if resp.cmd != CMD_START || resp.payload.first() != Some(&0x00) {
        bail!("START failed");
    }

    let (tcp_read, tcp_write) = tcp.into_split();
    let tcp_write = Arc::new(Mutex::new(tcp_write));

    let tcp_write_hb = tcp_write.clone();
    let heartbeat = tokio::spawn(async move {
        let pkt: [u8; 5] = [CMD_HEARTBEAT, 0x00, 0x00, 0x00, 0x00];
        {
            let mut w = tcp_write_hb.lock().await;
            let _ = w.write_all(&pkt).await;
        }
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let mut w = tcp_write_hb.lock().await;
            if w.write_all(&pkt).await.is_err() { break; }
        }
    });

    let drain = tokio::spawn(async move {
        let mut tcp_read = tcp_read;
        let mut buf = [0u8; 1024];
        loop {
            match tcp_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    Ok((
        ConnectedSession {
            tcp_write,
            udp,
            recv_buf: vec![0u8; 4096],
        },
        drain,
        heartbeat,
    ))
}
