mod archiver;
mod config;
mod deployer;
mod logger;
mod sftp_client;
mod ssh_client;

use clap::{Parser, Subcommand};
use log::info;
use md5;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::process;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(
    name = "deploy",
    version,
    about = "A minimal CLI that defaults to 'deploy'"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run deployment
    Deploy,
    /// Process files and detect changes
    File,
    /// Switch Maven settings file
    Mvn {
        /// Settings profile name (e.g., yjd, zzdt). If not specified, uses default settings.xml
        profile: Option<String>,
    },
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
        Commands::File => {
            // 加载配置
            let cfg = config::load_config().unwrap_or_else(|e| {
                eprintln!("加载配置失败: {}", e);
                std::process::exit(1);
            });

            // 获取配置文件所在目录，缓存文件存储在此目录下
            let config_dir = config::get_config_dir().unwrap_or_else(|e| {
                eprintln!("获取配置目录失败: {}", e);
                std::process::exit(1);
            });

            let target_dir = &cfg.paths.file_target_dir;
            let cache_file = config_dir.join("md5_cache.json");

            // 读取之前的MD5缓存
            let old_md5: HashMap<String, String> = if fs::metadata(&cache_file).is_ok() {
                let data = fs::read_to_string(&cache_file).unwrap_or_default();
                serde_json::from_str(&data).unwrap_or_default()
            } else {
                HashMap::new()
            };

            // 计算当前所有文件的MD5
            let mut current_md5: HashMap<String, String> = HashMap::new();
            for entry in WalkDir::new(target_dir).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let path = entry.path().to_string_lossy().to_string();
                    // 排除temp目录和md5_cache.json
                    if !path.contains("/temp/") && !path.ends_with("md5_cache.json") {
                        if let Ok(content) = fs::read(&path) {
                            let digest = format!("{:x}", md5::compute(&content));
                            current_md5.insert(path, digest);
                        }
                    }
                }
            }

            // 找出变动的文件（MD5不同或新增）
            let mut changed_files: Vec<String> = Vec::new();
            for (path, md5) in &current_md5 {
                if let Some(old_md5_val) = old_md5.get(path) {
                    if old_md5_val != md5 {
                        changed_files.push(path.clone());
                    }
                } else {
                    changed_files.push(path.clone());
                }
            }

            // 创建temp目录（先清空）
            let temp_dir = format!("{}/temp", target_dir);
            let _ = fs::remove_dir_all(&temp_dir); // 忽略错误，如果不存在
            if let Err(e) = fs::create_dir_all(&temp_dir) {
                eprintln!("创建temp目录失败: {}", e);
                std::process::exit(1);
            }

            // 复制变动的文件到temp目录（扁平化）
            let changed_count = changed_files.len();
            for path in changed_files {
                if let Some(file_name) = std::path::Path::new(&path).file_name() {
                    let file_name_str = file_name.to_string_lossy().to_string();
                    let dest = format!("{}/{}", temp_dir, file_name_str);
                    if let Err(e) = fs::copy(&path, &dest) {
                        eprintln!("复制文件失败 {} -> {}: {}", path, dest, e);
                    }
                }
            }

            // 保存当前MD5到缓存
            if let Ok(json) = serde_json::to_string(&current_md5) {
                if let Err(e) = fs::write(&cache_file, json) {
                    eprintln!("保存MD5缓存失败: {}", e);
                }
            }

            // 在macOS上打开temp目录
            if cfg!(target_os = "macos") {
                let _ = process::Command::new("open").arg(&temp_dir).spawn();
            }

            info!("处理了 {} 个变动的文件", changed_count);
        }
        Commands::Mvn { profile } => {
            // 加载配置
            let cfg = config::load_config().unwrap_or_else(|e| {
                eprintln!("加载配置失败: {}", e);
                std::process::exit(1);
            });

            // 获取 Maven 配置，如果没有配置则使用默认路径
            let maven_home = if let Some(ref maven_config) = cfg.maven {
                &maven_config.maven_home
            } else {
                "/Users/yuxuan/SoftWare/maven/apache-maven-3.6.3"
            };

            let settings_base_dir = format!("{}/conf/settings", maven_home);
            let target_settings = format!("{}/conf/settings.xml", maven_home);

            // 删除原来的 settings.xml
            if fs::metadata(&target_settings).is_ok() {
                if let Err(e) = fs::remove_file(&target_settings) {
                    eprintln!("删除原 settings.xml 失败: {}", e);
                    std::process::exit(1);
                }
                info!("已删除原 settings.xml");
            }

            // 根据参数选择源文件
            let source_file = if let Some(ref profile_name) = profile {
                format!("{}/settings-{}.xml", settings_base_dir, profile_name)
            } else {
                format!("{}/settings.xml", settings_base_dir)
            };

            // 检查源文件是否存在
            if !fs::metadata(&source_file).is_ok() {
                eprintln!("源文件不存在: {}", source_file);
                std::process::exit(1);
            }

            // 复制文件
            if let Err(e) = fs::copy(&source_file, &target_settings) {
                eprintln!("复制文件失败 {} -> {}: {}", source_file, target_settings, e);
                std::process::exit(1);
            }

            if let Some(ref profile_name) = profile {
                info!(
                    "已切换到 Maven 配置: {} (从 settings-{}.xml)",
                    profile_name, profile_name
                );
            } else {
                info!("已切换到 Maven 默认配置 (从 settings.xml)");
            }
        }
    }
}
