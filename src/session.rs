use std::io::{Read, Write};
use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use anyhow::{Context, Result, anyhow};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use russh::ChannelMsg;
use russh::client::{self, Handle};
use russh::keys::known_hosts::{check_known_hosts_path, learn_known_hosts_path};
use russh::keys::{PrivateKeyWithHashAlg, decode_secret_key};
use russh::{Channel, Disconnect};
use russh_sftp::client::SftpSession;
use tokio::io::copy_bidirectional;
use tokio::net::TcpListener;
use tokio::select;
use tokio::sync::{mpsc, watch};

use crate::app::default_local_shell_path;
use crate::models::{
    Connection, ConnectionType, Identity, Key as SshKey, PortForward, PortForwardType, SftpEntry,
};

#[derive(Debug, Clone)]
pub struct SessionHandle {
    pub command_tx: mpsc::UnboundedSender<SessionCommand>,
}

pub struct SftpHandle {
    _client: Arc<Handle<ClientHandler>>,
    session: Arc<SftpSession>,
}

pub struct PortForwardHandle {
    stop_tx: watch::Sender<bool>,
}

impl std::fmt::Debug for SftpHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SftpHandle").finish_non_exhaustive()
    }
}

impl Clone for SftpHandle {
    fn clone(&self) -> Self {
        Self {
            _client: Arc::clone(&self._client),
            session: Arc::clone(&self.session),
        }
    }
}

impl std::fmt::Debug for PortForwardHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortForwardHandle").finish_non_exhaustive()
    }
}

impl Clone for PortForwardHandle {
    fn clone(&self) -> Self {
        Self {
            stop_tx: self.stop_tx.clone(),
        }
    }
}

impl PortForwardHandle {
    pub fn stop(&self) {
        let _ = self.stop_tx.send(true);
    }
}

#[derive(Debug, Clone)]
pub enum SessionCommand {
    Input(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    Disconnect(String),
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    Connected { description: String },
    Output(Vec<u8>),
    Status(String),
    Error(String),
    Disconnected(String),
}

#[derive(Debug, Clone)]
pub struct ConnectionTarget {
    pub connection: Connection,
    pub key: Option<SshKey>,
    pub identity: Option<Identity>,
    pub known_hosts_path: std::path::PathBuf,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug)]
struct ClientHandler {
    host: String,
    port: u16,
    known_hosts_path: std::path::PathBuf,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        match check_known_hosts_path(
            &self.host,
            self.port,
            server_public_key,
            &self.known_hosts_path,
        ) {
            Ok(true) => Ok(true),
            Ok(false) => {
                learn_known_hosts_path(
                    &self.host,
                    self.port,
                    server_public_key,
                    &self.known_hosts_path,
                )?;
                Ok(true)
            }
            Err(error) => Err(error.into()),
        }
    }
}

pub async fn connect_target(
    target: ConnectionTarget,
    event_tx: mpsc::UnboundedSender<SessionEvent>,
) -> std::result::Result<SessionHandle, String> {
    match target.connection.connection_type {
        ConnectionType::Ssh => connect_ssh_session(target, event_tx)
            .await
            .map_err(|error| format!("SSH 连接失败: {error:#}")),
        ConnectionType::Local => connect_local_session(target, event_tx)
            .map_err(|error| format!("本地终端启动失败: {error:#}")),
        ConnectionType::Serial => connect_serial_session(target, event_tx)
            .map_err(|error| format!("Serial 连接失败: {error:#}")),
    }
}

pub async fn connect_sftp_target(
    target: ConnectionTarget,
) -> std::result::Result<SftpHandle, String> {
    let mut handle = open_client(&target)
        .await
        .map_err(|error| format!("SFTP 连接失败: {error:#}"))?;
    authenticate(&mut handle, &target)
        .await
        .map_err(|error| format!("SFTP 认证失败: {error:#}"))?;

    let channel = handle
        .channel_open_session()
        .await
        .map_err(|error| format!("打开 SFTP channel 失败: {error:#}"))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|error| format!("请求 SFTP subsystem 失败: {error:#}"))?;
    let session = SftpSession::new(channel.into_stream())
        .await
        .map_err(|error| format!("初始化 SFTP 会话失败: {error:#}"))?;

    Ok(SftpHandle {
        _client: Arc::new(handle),
        session: Arc::new(session),
    })
}

pub async fn sftp_list_dir(
    handle: SftpHandle,
    path: String,
) -> std::result::Result<Vec<SftpEntry>, String> {
    let mut entries = handle
        .session
        .read_dir(path.clone())
        .await
        .map_err(|error| format!("读取远端目录失败: {error:#}"))?
        .map(|entry| {
            let metadata = entry.metadata();
            SftpEntry {
                path: normalize_remote_child(&path, &entry.file_name()),
                name: entry.file_name(),
                is_dir: metadata.is_dir(),
                size: metadata.size.unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
    });

    Ok(entries)
}

pub async fn sftp_read_file_preview(
    handle: SftpHandle,
    path: String,
) -> std::result::Result<String, String> {
    let bytes = handle
        .session
        .read(path)
        .await
        .map_err(|error| format!("读取远端文件失败: {error:#}"))?;
    let preview = if bytes.len() > 128 * 1024 {
        &bytes[..128 * 1024]
    } else {
        &bytes
    };
    Ok(String::from_utf8_lossy(preview).to_string())
}

pub async fn start_port_forward(
    target: ConnectionTarget,
    forward: PortForward,
) -> std::result::Result<PortForwardHandle, String> {
    if forward.forward_type != PortForwardType::Local {
        return Err("当前版本仅支持 Local port forwarding".into());
    }

    let mut handle = open_client(&target)
        .await
        .map_err(|error| format!("端口转发连接失败: {error:#}"))?;
    authenticate(&mut handle, &target)
        .await
        .map_err(|error| format!("端口转发认证失败: {error:#}"))?;

    let listener = TcpListener::bind(format!("{}:{}", forward.bind_address, forward.bind_port))
        .await
        .map_err(|error| format!("监听本地端口失败: {error}"))?;

    let (stop_tx, mut stop_rx) = watch::channel(false);
    let bind_address = forward.bind_address.clone();
    let destination_host = forward.destination_host.clone();
    let destination_port = forward.destination_port as u32;
    let client = Arc::new(handle);

    tokio::spawn(async move {
        loop {
            select! {
                _ = stop_rx.changed() => {
                    if *stop_rx.borrow() {
                        break;
                    }
                }
                accepted = listener.accept() => {
                    let Ok((mut socket, remote_addr)) = accepted else {
                        break;
                    };
                    let client = Arc::clone(&client);
                    let destination_host = destination_host.clone();
                    let originator_ip = remote_addr.ip().to_string();
                    let originator_port = remote_addr.port() as u32;
                    tokio::spawn(async move {
                        let Ok(channel) = client
                            .channel_open_direct_tcpip(
                                destination_host,
                                destination_port,
                                originator_ip,
                                originator_port,
                            )
                            .await else {
                                return;
                            };
                        let mut stream = channel.into_stream();
                        let _ = copy_bidirectional(&mut socket, &mut stream).await;
                    });
                }
            }
        }

        let _ = client
            .disconnect(
                Disconnect::ByApplication,
                &format!("stop local forward on {}", bind_address),
                "zh-CN",
            )
            .await;
    });

    Ok(PortForwardHandle { stop_tx })
}

fn connect_local_session(
    target: ConnectionTarget,
    event_tx: mpsc::UnboundedSender<SessionEvent>,
) -> Result<SessionHandle> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: target.rows,
        cols: target.cols,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell = resolved_local_shell_path(&target.connection);
    let work_dir = resolved_local_work_dir(&target.connection);
    let mut command = build_local_command(&target.connection, &shell, &work_dir)?;
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    command.env("TERM_PROGRAM", "Timon");
    command.env("SHELL", &shell);

    if Path::new(&work_dir).is_dir() {
        command.cwd(&work_dir);
    }

    let mut child = pair.slave.spawn_command(command)?;
    let mut reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;
    let master = Arc::new(Mutex::new(pair.master));
    let writer = Arc::new(Mutex::new(writer));

    let startup_command = target.connection.startup_command.clone();
    let command_name = target.connection.name.clone();
    let (command_tx, mut command_rx) = mpsc::unbounded_channel();

    let _ = event_tx.send(SessionEvent::Connected {
        description: format!("Local PTY: {command_name}"),
    });

    if !startup_command.trim().is_empty() {
        if let Ok(mut guard) = writer.lock() {
            let _ = guard.write_all(startup_command.as_bytes());
            let _ = guard.write_all(b"\n");
        }
    }

    {
        let event_tx = event_tx.clone();
        thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        let _ =
                            event_tx.send(SessionEvent::Disconnected("本地 shell 已结束".into()));
                        break;
                    }
                    Ok(read) => {
                        let _ = event_tx.send(SessionEvent::Output(buffer[..read].to_vec()));
                    }
                    Err(error) => {
                        let _ = event_tx
                            .send(SessionEvent::Error(format!("读取本地 PTY 失败: {error}")));
                        break;
                    }
                }
            }
        });
    }

    tokio::spawn(async move {
        while let Some(command) = command_rx.recv().await {
            match command {
                SessionCommand::Input(data) => {
                    if let Ok(mut guard) = writer.lock() {
                        let _ = guard.write_all(&data);
                        let _ = guard.flush();
                    }
                }
                SessionCommand::Resize { cols, rows } => {
                    if let Ok(guard) = master.lock() {
                        let _ = guard.resize(PtySize {
                            rows,
                            cols,
                            pixel_width: 0,
                            pixel_height: 0,
                        });
                    }
                }
                SessionCommand::Disconnect(reason) => {
                    let _ = child.kill();
                    let _ = event_tx.send(SessionEvent::Disconnected(reason));
                    return;
                }
            }
        }
    });

    Ok(SessionHandle { command_tx })
}

#[cfg(unix)]
fn connect_serial_session(
    target: ConnectionTarget,
    event_tx: mpsc::UnboundedSender<SessionEvent>,
) -> Result<SessionHandle> {
    let device = target.connection.serial_port.trim();
    if device.is_empty() {
        return Err(anyhow!("串口设备路径不能为空"));
    }

    let baud_rate = target.connection.baud_rate;
    let port = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
        .open(device)
        .with_context(|| format!("无法打开串口设备 {device}"))?;

    configure_serial_port(port.as_raw_fd(), baud_rate)?;

    let mut reader = port
        .try_clone()
        .with_context(|| format!("无法克隆串口 reader {device}"))?;
    let writer = Arc::new(Mutex::new(port));
    let stop = Arc::new(AtomicBool::new(false));
    let (command_tx, mut command_rx) = mpsc::unbounded_channel();

    let _ = event_tx.send(SessionEvent::Connected {
        description: format!("{device} @ {baud_rate}"),
    });

    if !target.connection.startup_command.trim().is_empty() {
        let startup_command = target.connection.startup_command.clone();
        if let Ok(mut guard) = writer.lock() {
            let _ = guard.write_all(startup_command.as_bytes());
            let _ = guard.write_all(b"\n");
            let _ = guard.flush();
        }
    }

    {
        let event_tx = event_tx.clone();
        let stop = Arc::clone(&stop);
        thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            while !stop.load(Ordering::Relaxed) {
                match reader.read(&mut buffer) {
                    Ok(0) => thread::sleep(Duration::from_millis(10)),
                    Ok(read) => {
                        let _ = event_tx.send(SessionEvent::Output(buffer[..read].to_vec()));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(error) => {
                        let _ =
                            event_tx.send(SessionEvent::Error(format!("读取串口失败: {error}")));
                        break;
                    }
                }
            }
        });
    }

    tokio::spawn(async move {
        while let Some(command) = command_rx.recv().await {
            match command {
                SessionCommand::Input(data) => {
                    if let Ok(mut guard) = writer.lock() {
                        let _ = guard.write_all(&data);
                        let _ = guard.flush();
                    }
                }
                SessionCommand::Resize { .. } => {}
                SessionCommand::Disconnect(reason) => {
                    stop.store(true, Ordering::Relaxed);
                    let _ = event_tx.send(SessionEvent::Disconnected(reason));
                    return;
                }
            }
        }

        stop.store(true, Ordering::Relaxed);
    });

    Ok(SessionHandle { command_tx })
}

#[cfg(not(unix))]
fn connect_serial_session(
    _target: ConnectionTarget,
    _event_tx: mpsc::UnboundedSender<SessionEvent>,
) -> Result<SessionHandle> {
    Err(anyhow!("当前平台暂不支持 Serial 连接"))
}

#[cfg(unix)]
fn configure_serial_port(fd: std::os::fd::RawFd, baud_rate: i64) -> Result<()> {
    let speed = serial_baud_rate(baud_rate)?;

    unsafe {
        let mut settings = std::mem::zeroed::<libc::termios>();
        if libc::tcgetattr(fd, &mut settings) != 0 {
            return Err(std::io::Error::last_os_error()).context("读取串口参数失败");
        }

        libc::cfmakeraw(&mut settings);

        if libc::cfsetispeed(&mut settings, speed) != 0
            || libc::cfsetospeed(&mut settings, speed) != 0
        {
            return Err(std::io::Error::last_os_error()).context("设置串口波特率失败");
        }

        settings.c_cflag |= libc::CLOCAL | libc::CREAD;
        settings.c_cflag &= !libc::PARENB;
        settings.c_cflag &= !libc::CSTOPB;
        settings.c_cflag &= !libc::CSIZE;
        settings.c_cflag |= libc::CS8;
        settings.c_iflag &= !(libc::IXON | libc::IXOFF | libc::IXANY);
        settings.c_cc[libc::VMIN] = 0;
        settings.c_cc[libc::VTIME] = 1;

        if libc::tcsetattr(fd, libc::TCSANOW, &settings) != 0 {
            return Err(std::io::Error::last_os_error()).context("应用串口参数失败");
        }
    }

    Ok(())
}

#[cfg(unix)]
fn serial_baud_rate(baud_rate: i64) -> Result<libc::speed_t> {
    match baud_rate {
        50 => Ok(libc::B50),
        75 => Ok(libc::B75),
        110 => Ok(libc::B110),
        134 => Ok(libc::B134),
        150 => Ok(libc::B150),
        200 => Ok(libc::B200),
        300 => Ok(libc::B300),
        600 => Ok(libc::B600),
        1200 => Ok(libc::B1200),
        1800 => Ok(libc::B1800),
        2400 => Ok(libc::B2400),
        4800 => Ok(libc::B4800),
        9600 => Ok(libc::B9600),
        19200 => Ok(libc::B19200),
        38400 => Ok(libc::B38400),
        57600 => Ok(libc::B57600),
        115200 => Ok(libc::B115200),
        230400 => Ok(libc::B230400),
        _ => Err(anyhow!(
            "不支持的串口波特率 {baud_rate}，当前支持 50-230400"
        )),
    }
}

fn resolved_local_shell_path(connection: &Connection) -> String {
    let configured = connection.shell_path.trim();
    if configured.is_empty() {
        default_local_shell_path()
    } else {
        configured.to_string()
    }
}

fn resolved_local_work_dir(connection: &Connection) -> String {
    connection.work_dir.trim().to_string()
}

#[cfg(target_os = "macos")]
fn build_local_command(
    connection: &Connection,
    shell: &str,
    work_dir: &str,
) -> Result<CommandBuilder> {
    let user = std::env::var("USER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("USERNAME")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "admin".into());
    if connection.shell_path.trim().is_empty() && work_dir.trim().is_empty() {
        let mut command = CommandBuilder::new("/usr/bin/login");
        command.args(["-flp", &user]);
        return Ok(command);
    }

    let mut command = CommandBuilder::new("/usr/bin/login");
    let exec = macos_login_exec(shell, work_dir);
    command.args(["-flp", &user, "/bin/zsh", "-fc", &exec]);
    Ok(command)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn build_local_command(
    _connection: &Connection,
    shell: &str,
    work_dir: &str,
) -> Result<CommandBuilder> {
    let mut command = CommandBuilder::new(shell);
    if work_dir.trim().is_empty() {
        command.arg("-l");
    } else {
        command.cwd(work_dir);
    }
    Ok(command)
}

#[cfg(windows)]
fn build_local_command(
    _connection: &Connection,
    shell: &str,
    work_dir: &str,
) -> Result<CommandBuilder> {
    let mut command = CommandBuilder::new(shell);
    if !work_dir.trim().is_empty() {
        command.cwd(work_dir);
    }
    Ok(command)
}

#[cfg(target_os = "macos")]
fn single_quote_for_sh(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\''"#))
}

#[cfg(target_os = "macos")]
fn macos_login_exec(shell: &str, work_dir: &str) -> String {
    let shell_name = Path::new(shell)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("shell");
    let mut command = String::new();
    if Path::new(work_dir).is_dir() {
        command.push_str("cd -- ");
        command.push_str(&single_quote_for_sh(work_dir));
        command.push_str(" && ");
    }
    command.push_str("exec -a -");
    command.push_str(shell_name);
    command.push(' ');
    command.push_str(&single_quote_for_sh(shell));
    command.push_str(" -l");
    command
}

async fn connect_ssh_session(
    target: ConnectionTarget,
    event_tx: mpsc::UnboundedSender<SessionEvent>,
) -> Result<SessionHandle> {
    let mut handle = open_client(&target).await?;
    authenticate(&mut handle, &target).await?;

    let shell = open_shell(&handle, target.cols, target.rows).await?;

    if !target.connection.startup_command.trim().is_empty() {
        let _ = event_tx.send(SessionEvent::Status(
            "SSH 已连接，准备执行 startup command".into(),
        ));
    }

    let _ = probe_sftp(&handle).await;

    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let description = format!(
        "{}@{}:{}",
        effective_username(&target),
        target.connection.host,
        target.connection.port
    );
    let _ = event_tx.send(SessionEvent::Connected { description });

    tokio::spawn(run_ssh_loop(
        handle,
        shell,
        command_rx,
        event_tx.clone(),
        target.connection.startup_command.clone(),
    ));

    Ok(SessionHandle { command_tx })
}

async fn open_client(target: &ConnectionTarget) -> Result<Handle<ClientHandler>> {
    let config = Arc::new(client::Config::default());
    let address = format!("{}:{}", target.connection.host, target.connection.port);
    let handler = ClientHandler {
        host: target.connection.host.clone(),
        port: target.connection.port as u16,
        known_hosts_path: target.known_hosts_path.clone(),
    };

    client::connect(config, address, handler)
        .await
        .map_err(Into::into)
}

async fn authenticate(handle: &mut Handle<ClientHandler>, target: &ConnectionTarget) -> Result<()> {
    let username = effective_username(target);

    let auth_result = if let Some(key) = effective_private_key(target)? {
        let hash_alg = handle.best_supported_rsa_hash().await?.flatten();
        handle
            .authenticate_publickey(
                username,
                PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg),
            )
            .await?
    } else if let Some(password) = effective_password(target) {
        handle.authenticate_password(username, password).await?
    } else {
        return Err(anyhow!("缺少密码或私钥"));
    };

    if auth_result.success() {
        Ok(())
    } else {
        Err(anyhow!("服务端拒绝认证"))
    }
}

async fn open_shell(
    handle: &Handle<ClientHandler>,
    cols: u16,
    rows: u16,
) -> Result<Channel<russh::client::Msg>> {
    let channel = handle.channel_open_session().await?;
    channel
        .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
        .await?;
    channel.request_shell(true).await?;
    Ok(channel)
}

async fn probe_sftp(handle: &Handle<ClientHandler>) -> Result<()> {
    let channel = handle.channel_open_session().await?;
    channel.request_subsystem(true, "sftp").await?;
    let _sftp = SftpSession::new(channel.into_stream()).await?;
    Ok(())
}

async fn run_ssh_loop(
    handle: Handle<ClientHandler>,
    mut channel: Channel<russh::client::Msg>,
    mut command_rx: mpsc::UnboundedReceiver<SessionCommand>,
    event_tx: mpsc::UnboundedSender<SessionEvent>,
    startup_command: String,
) {
    if !startup_command.trim().is_empty() {
        let _ = channel.data(startup_command.as_bytes()).await;
        let _ = channel.data(&b"\n"[..]).await;
    }

    loop {
        select! {
            command = command_rx.recv() => {
                match command {
                    Some(SessionCommand::Input(data)) => {
                        if let Err(error) = channel.data(&data[..]).await {
                            let _ = event_tx.send(SessionEvent::Error(format!("发送 SSH 输入失败: {error:#}")));
                        }
                    }
                    Some(SessionCommand::Resize { cols, rows }) => {
                        if let Err(error) = channel.window_change(cols as u32, rows as u32, 0, 0).await {
                            let _ = event_tx.send(SessionEvent::Error(format!("调整 SSH PTY 大小失败: {error:#}")));
                        }
                    }
                    Some(SessionCommand::Disconnect(reason)) => {
                        let _ = channel.eof().await;
                        let _ = channel.close().await;
                        let _ = handle.disconnect(Disconnect::ByApplication, &reason, "zh-CN").await;
                        let _ = event_tx.send(SessionEvent::Disconnected(reason));
                        return;
                    }
                    None => break,
                }
            }
            message = channel.wait() => {
                match message {
                    Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                        let _ = event_tx.send(SessionEvent::Output(data.to_vec()));
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        let _ = event_tx.send(SessionEvent::Status(format!("远端进程退出，exit code={exit_status}")));
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) => {
                        let reason = "远端 SSH 会话已关闭".to_string();
                        let _ = event_tx.send(SessionEvent::Disconnected(reason.clone()));
                        let _ = handle.disconnect(Disconnect::ByApplication, &reason, "zh-CN").await;
                        return;
                    }
                    Some(_) => {}
                    None => {
                        let reason = "SSH channel 已关闭".to_string();
                        let _ = event_tx.send(SessionEvent::Disconnected(reason.clone()));
                        let _ = handle.disconnect(Disconnect::ByApplication, &reason, "zh-CN").await;
                        return;
                    }
                }
            }
        }
    }

    let reason = "SSH 会话命令通道已关闭".to_string();
    let _ = handle
        .disconnect(Disconnect::ByApplication, &reason, "zh-CN")
        .await;
    let _ = event_tx.send(SessionEvent::Disconnected(reason));
}

fn effective_username(target: &ConnectionTarget) -> String {
    if !target.connection.username.trim().is_empty() {
        target.connection.username.clone()
    } else {
        target
            .identity
            .as_ref()
            .map(|identity| identity.username.clone())
            .unwrap_or_default()
    }
}

fn effective_password(target: &ConnectionTarget) -> Option<String> {
    if !target.connection.password.is_empty() {
        Some(target.connection.password.clone())
    } else {
        target
            .identity
            .as_ref()
            .and_then(|identity| (!identity.password.is_empty()).then(|| identity.password.clone()))
    }
}

fn effective_private_key(target: &ConnectionTarget) -> Result<Option<russh::keys::PrivateKey>> {
    if let Some(key) = effective_key(target) {
        if !key.private_key.trim().is_empty() {
            return decode_secret_key(&key.private_key, None)
                .map(Some)
                .context("解析 key 私钥失败");
        }
    }

    Ok(None)
}

fn effective_key(target: &ConnectionTarget) -> Option<&SshKey> {
    let connection_key = target.connection.key_id;
    let identity_key = target
        .identity
        .as_ref()
        .and_then(|identity| identity.key_id);

    match (connection_key, identity_key, target.key.as_ref()) {
        (Some(_), _, key) => key,
        (None, Some(_), key) => key,
        _ => target.key.as_ref(),
    }
}

fn normalize_remote_child(parent: &str, child: &str) -> String {
    if parent == "/" {
        format!("/{}", child)
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), child)
    }
}
