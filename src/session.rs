use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, Result, anyhow};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use russh::ChannelMsg;
use russh::client::{self, Handle};
use russh::keys::known_hosts::{check_known_hosts_path, learn_known_hosts_path};
use russh::keys::{PrivateKeyWithHashAlg, decode_secret_key};
use russh::{Channel, Disconnect};
use russh_sftp::client::SftpSession;
use tokio::select;
use tokio::sync::mpsc;

use crate::models::{Certificate, Identity, Profile, ProfileType};

#[derive(Debug, Clone)]
pub struct SessionHandle {
    pub command_tx: mpsc::UnboundedSender<SessionCommand>,
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
    pub profile: Profile,
    pub certificate: Option<Certificate>,
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
    match target.profile.profile_type {
        ProfileType::Ssh => connect_ssh_session(target, event_tx)
            .await
            .map_err(|error| format!("SSH 连接失败: {error:#}")),
        ProfileType::Local => connect_local_session(target, event_tx)
            .map_err(|error| format!("本地终端启动失败: {error:#}")),
    }
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

    // let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    // let command = CommandBuilder::new(shell);

    // 获取当前用户名
    let user = std::env::var("USER").unwrap_or_else(|_| "admin".into());
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());

    // 使用 /usr/bin/login 启动
    // -f: 跳过密码验证（因为你已经在用户会话中了）
    // -p: 保留环境变量
    let mut command = CommandBuilder::new("/usr/bin/login");
    command.args(&["-fp", &user, &shell]);

    let mut child = pair.slave.spawn_command(command)?;
    let mut reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;
    let master = Arc::new(Mutex::new(pair.master));
    let writer = Arc::new(Mutex::new(writer));

    let startup_command = target.profile.startup_command.clone();
    let command_name = target.profile.name.clone();
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

async fn connect_ssh_session(
    target: ConnectionTarget,
    event_tx: mpsc::UnboundedSender<SessionEvent>,
) -> Result<SessionHandle> {
    let mut handle = open_client(&target).await?;
    authenticate(&mut handle, &target).await?;

    let shell = open_shell(&handle, target.cols, target.rows).await?;

    if !target.profile.startup_command.trim().is_empty() {
        let _ = event_tx.send(SessionEvent::Status(
            "SSH 已连接，准备执行 startup command".into(),
        ));
    }

    let _ = probe_sftp(&handle).await;

    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let description = format!(
        "{}@{}:{}",
        effective_username(&target),
        target.profile.host,
        target.profile.port
    );
    let _ = event_tx.send(SessionEvent::Connected { description });

    tokio::spawn(run_ssh_loop(
        handle,
        shell,
        command_rx,
        event_tx.clone(),
        target.profile.startup_command.clone(),
    ));

    Ok(SessionHandle { command_tx })
}

async fn open_client(target: &ConnectionTarget) -> Result<Handle<ClientHandler>> {
    let config = Arc::new(client::Config::default());
    let address = format!("{}:{}", target.profile.host, target.profile.port);
    let handler = ClientHandler {
        host: target.profile.host.clone(),
        port: target.profile.port as u16,
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
    if !target.profile.username.trim().is_empty() {
        target.profile.username.clone()
    } else {
        target
            .identity
            .as_ref()
            .map(|identity| identity.username.clone())
            .unwrap_or_default()
    }
}

fn effective_password(target: &ConnectionTarget) -> Option<String> {
    if !target.profile.password.is_empty() {
        Some(target.profile.password.clone())
    } else {
        target
            .identity
            .as_ref()
            .and_then(|identity| (!identity.password.is_empty()).then(|| identity.password.clone()))
    }
}

fn effective_private_key(target: &ConnectionTarget) -> Result<Option<russh::keys::PrivateKey>> {
    if let Some(certificate) = effective_certificate(target) {
        if !certificate.private_key.trim().is_empty() {
            return decode_secret_key(&certificate.private_key, None)
                .map(Some)
                .context("解析证书私钥失败");
        }
    }

    Ok(None)
}

fn effective_certificate(target: &ConnectionTarget) -> Option<&Certificate> {
    let profile_certificate = target.profile.certificate_id;
    let identity_certificate = target
        .identity
        .as_ref()
        .and_then(|identity| identity.certificate_id);

    match (
        profile_certificate,
        identity_certificate,
        target.certificate.as_ref(),
    ) {
        (Some(_), _, certificate) => certificate,
        (None, Some(_), certificate) => certificate,
        _ => target.certificate.as_ref(),
    }
}
