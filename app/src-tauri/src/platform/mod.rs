// Use cfg_if to better manage multiple platform-specific modules
use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(target_os = "windows")] {
        mod windows;
        pub use windows::*;
    } else if #[cfg(target_os = "macos")] {
        mod macos;
        pub use macos::*;
    } else if #[cfg(target_os = "linux")] {
        mod linux;
        pub use linux::*;
    }
}
