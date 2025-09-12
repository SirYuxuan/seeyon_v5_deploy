use ssh2::{OpenFlags, OpenType, Sftp};
use std::fs::File;
use std::path::Path;

/// 使用 SFTP 将本地文件上传到远端指定路径
pub fn sftp_upload_file(sftp: &Sftp, local_path: &Path, remote_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut local_file = File::open(local_path)?;

    let flags = OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNCATE;
    let mode: i32 = 0o644;
    let mut remote_file = sftp.open_mode(Path::new(remote_path), flags, mode, OpenType::File)?;

    std::io::copy(&mut local_file, &mut remote_file)?;
    Ok(())
}
