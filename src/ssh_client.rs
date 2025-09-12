use crate::config::SshConfig;
use ssh2::Session;
use std::net::TcpStream;
use std::time::Duration;
use std::io::{Read, BufRead, BufReader};
use log::{info, error};

/// 建立 SSH 连接并完成用户名/密码认证
pub fn connect_ssh(cfg: &SshConfig) -> Result<Session, Box<dyn std::error::Error>> {
    let addr = format!("{}:{}", cfg.host, cfg.port);
    let stream = TcpStream::connect(&addr)?;
    if let Some(secs) = cfg.timeout_secs {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(secs)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(secs)));
    }

    let mut sess = Session::new()?;
    sess.set_tcp_stream(stream);
    sess.handshake()?;
    sess.userauth_password(&cfg.username, &cfg.password)?;

    if !sess.authenticated() {
        return Err("SSH未认证".into());
    }
    Ok(sess)
}

/// 在远端执行命令并返回标准输出，非零退出码视为错误
pub fn exec(session: &Session, cmd: &str) -> Result<String, Box<dyn std::error::Error>> {
    // 打开会话通道
    let mut channel = session.channel_session()?;
    // 执行命令
    channel.exec(cmd)?;

    // 读取标准输出
    let mut stdout = String::new();
    channel.read_to_string(&mut stdout)?;

    // 读取标准错误
    let mut stderr = String::new();
    channel.stderr().read_to_string(&mut stderr)?;

    // 关闭并获取退出码
    channel.wait_close()?;
    let status = channel.exit_status()?;
    if status != 0 {
        let msg = format!("远端命令执行失败 ({}): {}\n{}", status, cmd, stderr);
        return Err(msg.into());
    }
    Ok(stdout)
}

/// 在远端执行命令并实时输出过程（逐行），非零退出码视为错误
pub fn exec_stream(session: &Session, cmd: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut channel = session.channel_session()?;
    channel.exec(cmd)?;

    // 实时读取标准输出
    {
        let stdout = channel.stream(0);
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line)?;
            if n == 0 { break; }
            let trimmed = line.trim_end_matches(['\n', '\r']);
            if trimmed.is_empty() { continue; }
            // 过滤掉 .DS_Store 的输出
            if trimmed.contains(".DS_Store") { continue; }
            info!("{}", trimmed);
        }
    }

    // 读取标准错误
    {
        let mut err_reader = BufReader::new(channel.stderr());
        let mut line = String::new();
        loop {
            line.clear();
            let n = err_reader.read_line(&mut line)?;
            if n == 0 { break; }
            let trimmed = line.trim_end_matches(['\n', '\r']);
            if trimmed.is_empty() { continue; }
            if trimmed.contains(".DS_Store") { continue; }
            error!("{}", trimmed);
        }
    }

    channel.wait_close()?;
    let status = channel.exit_status()?;
    if status != 0 {
        return Err(format!("远端命令执行失败 ({}): {}", status, cmd).into());
    }
    Ok(())
}
