use crate::{archiver, config::Config, sftp_client};
use log::info;
use ssh2::Session;
use std::path::PathBuf;

/// 执行通用的打包 → 上传 → 远端解压并删除压缩包 流程
pub fn deploy_assets(sess: &Session, cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    // 打包本地目录
    let apps_tar_gz = PathBuf::from("apps.tar.gz");
    let cfg_tar_gz = PathBuf::from("cfgHome.tar.gz");
    archiver::archive_directory(&cfg.paths.local_apps, &apps_tar_gz, None)?;
    info!("已打包: {:?}", apps_tar_gz);
    archiver::archive_directory(&cfg.paths.local_cfg_home, &cfg_tar_gz, None)?;
    info!("已打包: {:?}", cfg_tar_gz);

    // 上传文件
    let sftp = sess.sftp()?;
    sftp_client::sftp_upload_file(&sftp, &apps_tar_gz, &format!("{}/apps.tar.gz", cfg.paths.remote_apps))?;
    info!("已上传 apps.tar.gz 到 {}", cfg.paths.remote_apps);
    sftp_client::sftp_upload_file(&sftp, &cfg_tar_gz, &format!("{}/cfgHome.tar.gz", cfg.paths.remote_cfg_home))?;
    info!("已上传 cfgHome.tar.gz 到 {}", cfg.paths.remote_cfg_home);

    // 远端解压并覆盖，随后删除远端压缩包（显示过程，过滤 .DS_Store 由 exec_stream 处理）
    let cmd_apps = format!(
        "cd {} && tar -xzvf apps.tar.gz && rm -f apps.tar.gz",
        cfg.paths.remote_apps
    );
    crate::ssh_client::exec_stream(sess, &cmd_apps)?;
    info!("已在远端解压并删除: {}/apps.tar.gz", cfg.paths.remote_apps);

    let cmd_cfg = format!(
        "cd {} && tar -xzvf cfgHome.tar.gz && rm -f cfgHome.tar.gz",
        cfg.paths.remote_cfg_home
    );
    crate::ssh_client::exec_stream(sess, &cmd_cfg)?;
    info!("已在远端解压并删除: {}/cfgHome.tar.gz", cfg.paths.remote_cfg_home);

    // 删除本地压缩文件
    let _ = std::fs::remove_file(&apps_tar_gz);
    let _ = std::fs::remove_file(&cfg_tar_gz);

    Ok(())
}
