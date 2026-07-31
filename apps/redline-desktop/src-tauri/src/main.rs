//! 发布的那个**单 exe** 的入口。
//!
//! 一个二进制同时是 GUI 和 CLI，靠第一个参数分流：
//!
//! | 调用方式                       | 走哪边 |
//! |--------------------------------|--------|
//! | `redline`（双击）              | GUI，空白工作台 |
//! | `redline C:\包.zip`（文件关联）| GUI，直接打开这个文件 |
//! | `redline inspect a.docx`       | CLI |
//! | `redline --help`               | CLI |
//!
//! 为什么非要合成一个：要取代 WinRAR，就必须能被双击、能进右键菜单、能做文件关联，
//! 那就得是 GUI 子系统的 exe；但同一个能力又得能被脚本和别的 AI 无界面调用。
//! 分成两个 exe 的话，用户装完会有两个图标，而「哪个是哪个」永远解释不清。

// GUI 子系统：双击不弹控制台窗口。代价是从命令行调用时 stdout 没地方去，
// 所以下面 CLI 分支里要手动把父进程的控制台接回来。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if redline_cli::is_cli_invocation(&args) {
        attach_parent_console();
        std::process::exit(redline_cli::run());
    }

    // 剩下的都当 GUI：没参数就是空白工作台，有参数就当成要打开的文件
    redline_desktop_lib::run(args.into_iter().next());
}

/// 把父进程的控制台接回来，让 CLI 输出有地方去。
///
/// GUI 子系统的进程从 cmd/PowerShell 启动时，标准句柄不一定被继承；不接控制台的话
/// `println!` 全部丢进虚空 —— 用户会看到「命令跑完了什么都没输出」，比报错还难查。
///
/// **只补空缺的句柄，绝不覆盖已有的。** 一旦无条件把 stdout 指向 CONOUT$，
/// `redline formats > out.json` 这类重定向就会被冲掉：文件是空的，内容跑去了控制台。
/// 这个坑踩过一次——测试里 out.json 为零字节、退出码却是 0。
#[cfg(windows)]
fn attach_parent_console() {
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::Console::{
        AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };

    unsafe {
        // 没有父控制台（双击带参数启动）就什么都不做，别硬造一个窗口出来
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return;
        }

        let open_console = |name: &str| {
            let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null_mut(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };

        // 句柄已经指向某个真东西（管道、文件、控制台）就别动它
        let mut fill = |slot: u32, device: &str| {
            let existing = GetStdHandle(slot);
            if !existing.is_null() && existing != INVALID_HANDLE_VALUE {
                return;
            }
            let handle = open_console(device);
            if handle != INVALID_HANDLE_VALUE {
                SetStdHandle(slot, handle);
            }
        };

        fill(STD_OUTPUT_HANDLE, "CONOUT$");
        fill(STD_ERROR_HANDLE, "CONOUT$");
        fill(STD_INPUT_HANDLE, "CONIN$");
    }
}

#[cfg(not(windows))]
fn attach_parent_console() {
    // 别的平台没有 GUI/控制台子系统之分，标准句柄本来就是通的
}
