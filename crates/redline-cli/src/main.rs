//! 独立 CLI 二进制。发布形态是合并进 `redline.exe` 的单 exe，
//! 这个 bin 保留给「只要 CLI、不要 GUI 依赖」的场景（CI、服务器、体积敏感的分发）。

fn main() {
    std::process::exit(redline_cli::run())
}
