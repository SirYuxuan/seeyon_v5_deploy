mod config;
mod logger;
mod ssh_client;
mod archiver;
mod sftp_client;
mod deployer;

use clap::{Parser, Subcommand};
use log::info;
 
 

#[derive(Parser)]
#[command(name = "deploy", version, about = "A minimal CLI that defaults to 'deploy'")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run deployment
    Deploy,
    /// Restart service (current step: only run shutdown.sh)
    Restart,
}

fn main() {
    // 初始化日志
    logger::init_logger();

    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Deploy) {
        Commands::Deploy => {
            // 加载配置
            let cfg = config::load_config().unwrap_or_else(|e| {
                eprintln!("加载配置失败: {}", e);
                std::process::exit(1);
            });

            // 连接 SSH
            let sess = ssh_client::connect_ssh(&cfg.ssh).unwrap_or_else(|e| {
                eprintln!("SSH连接失败: {}", e);
                std::process::exit(1);
            });
            info!("已成功连接并认证到 {}:{}", cfg.ssh.host, cfg.ssh.port);

            // 通用部署过程：打包 → 上传 → 远端解压
            if let Err(e) = deployer::deploy_assets(&sess, &cfg) {
                eprintln!("部署流程失败: {}", e);
                std::process::exit(1);
            }

            // 可选：执行远端关停脚本（例如 shutdown.sh）
            if let Some(cmd) = cfg.shutdown_cmd.as_ref() {
                if !cmd.trim().is_empty() {
                    let run = format!("sh {}", cmd);
                    if let Err(e) = ssh_client::exec_stream(&sess, &run) {
                        eprintln!("执行关停脚本失败: {}", e);
                        std::process::exit(1);
                    }
                    info!("已执行关停脚本: {}", cmd);
                }
            }
        }
        Commands::Restart => {
            // 加载配置（主要用于 SSH 连接参数）
            let cfg = config::load_config().unwrap_or_else(|e| {
                eprintln!("加载配置失败: {}", e);
                std::process::exit(1);
            });

            // 连接 SSH
            let sess = ssh_client::connect_ssh(&cfg.ssh).unwrap_or_else(|e| {
                eprintln!("SSH连接失败: {}", e);
                std::process::exit(1);
            });
            info!("已成功连接并认证到 {}:{}", cfg.ssh.host, cfg.ssh.port);

            // 执行关停脚本：优先使用配置中的绝对路径；否则用 bash -lc 调用全局 shutdown.sh
            if let Some(cmd) = cfg.shutdown_cmd.as_ref() {
                let run = format!("sh {}", cmd);
                if let Err(e) = ssh_client::exec_stream(&sess, &run) {
                    eprintln!("执行关停脚本失败: {}", e);
                    std::process::exit(1);
                }
                info!("已执行关停脚本: {}", cmd);
            } else {
                // 使用登录 shell 以获取完整 PATH
                if let Err(e) = ssh_client::exec_stream(&sess, "bash -lc 'shutdown.sh'") {
                    eprintln!("执行 shutdown.sh 失败: {}", e);
                    std::process::exit(1);
                }
                info!("已执行 shutdown.sh");
            }

            // 通用部署过程：打包 → 上传 → 远端解压
            if let Err(e) = deployer::deploy_assets(&sess, &cfg) {
                eprintln!("部署流程失败: {}", e);
                std::process::exit(1);
            }

            // 关停完成后，执行启动脚本：优先使用配置中的绝对路径；否则用 bash -lc 调用全局 startup.sh
            if let Some(cmd) = cfg.startup_cmd.as_ref() {
                let run = format!("sh {}", cmd);
                if let Err(e) = ssh_client::exec_stream(&sess, &run) {
                    eprintln!("执行启动脚本失败: {}", e);
                    std::process::exit(1);
                }
                info!("已执行启动脚本: {}", cmd);
            } else {
                if let Err(e) = ssh_client::exec_stream(&sess, "bash -lc 'startup.sh'") {
                    eprintln!("执行 startup.sh 失败: {}", e);
                    std::process::exit(1);
                }
                info!("已执行 startup.sh");
            }

            // 启动后执行 showLog（若配置，优先使用；否则尝试全局 showLog.sh）
            if let Some(cmd) = cfg.showlog_cmd.as_ref() {
                let run = format!("sh {}", cmd);
                if let Err(e) = ssh_client::exec_stream(&sess, &run) {
                    eprintln!("执行 showLog 失败: {}", e);
                    std::process::exit(1);
                }
                info!("已执行 showLog: {}", cmd);
            } else {
                let _ = ssh_client::exec_stream(&sess, "bash -lc 'showLogs.sh'");
            }
        }
    }
}

