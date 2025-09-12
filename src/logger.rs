use chrono::Local;
use std::io::Write;

/// 初始化日志：时间 + 级别
pub fn init_logger() {
    let _ = env_logger::Builder::from_default_env()
        .format(|buf, record| {
            let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
            writeln!(buf, "{} {} - {}", ts, record.level(), record.args())
        })
        .filter_level(log::LevelFilter::Info)
        .try_init();
}
