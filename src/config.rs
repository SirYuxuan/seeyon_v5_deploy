use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// 配置结构：包含 SSH 参数与各路径
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub ssh: SshConfig,
    pub paths: PathsConfig,
    /// 可选的远端关停脚本命令
    pub shutdown_cmd: Option<String>,
    /// 可选的远端启动脚本命令
    pub startup_cmd: Option<String>,
    /// 可选的查看日志脚本命令
    pub showlog_cmd: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    /// 超时秒数（可选）
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PathsConfig {
    /// 本地 apps 目录
    pub local_apps: String,
    /// 本地 cfgHome 目录
    pub local_cfg_home: String,
    /// 远端 apps 目录
    pub remote_apps: String,
    /// 远端 cfgHome 目录
    pub remote_cfg_home: String,
    /// file指令的目标目录
    pub file_target_dir: String,
}

/// 从 deploy.toml 加载配置：优先当前工作目录，其次可执行文件所在目录
pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    // 1) 优先尝试当前工作目录下的 deploy.toml
    let cwd_path = Path::new("deploy.toml");
    let candidate_paths: Vec<PathBuf> = if cwd_path.exists() {
        vec![cwd_path.to_path_buf()]
    } else {
        // 2) 回退到可执行文件所在目录
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let mut v = Vec::new();
        if let Some(dir) = exe_dir {
            v.push(dir.join("deploy.toml"));
        }
        v
    };

    for p in candidate_paths {
        if p.exists() {
            let content = fs::read_to_string(&p)?;
            let cfg: Config = toml::from_str(&content)?;
            return Ok(cfg);
        }
    }

    Err("未找到配置文件 deploy.toml，请将其放在当前目录或可执行文件同目录".into())
}
