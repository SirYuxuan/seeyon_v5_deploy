use std::fs::{self, File};
use std::path::Path;
use flate2::write::GzEncoder;
use flate2::Compression;
use tar::Builder;
use walkdir::WalkDir;

/// 将指定目录打包为 tar.gz 文件
/// - src_dir: 要打包的目录
/// - output_tar_gz: 输出的 .tar.gz 文件路径
/// - root_name: 放入归档中的根目录名（为 None 时去掉多一级目录）
pub fn archive_directory(src_dir: &str, output_tar_gz: &Path, root_name: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    if output_tar_gz.exists() {
        fs::remove_file(output_tar_gz)?;
    }
    let file = File::create(output_tar_gz)?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar_builder = Builder::new(enc);

    let src_path = Path::new(src_dir);
    if !src_path.exists() {
        return Err(format!("源目录不存在: {}", src_dir).into());
    }

    match root_name {
        Some(name) => {
            tar_builder.append_dir_all(name, src_path)?;
        }
        None => {
            for entry in WalkDir::new(src_path).into_iter().filter_map(Result::ok) {
                let path = entry.path();
                let rel = match path.strip_prefix(src_path) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                if rel.as_os_str().is_empty() {
                    continue;
                }
                if entry.file_type().is_dir() {
                    tar_builder.append_dir(rel, path)?;
                } else if entry.file_type().is_file() {
                    let mut f = File::open(path)?;
                    tar_builder.append_file(rel, &mut f)?;
                }
            }
        }
    }

    tar_builder.finish()?;
    Ok(())
}
