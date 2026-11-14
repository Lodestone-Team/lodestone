// Delay-loads the Windows DLLs the core rarely or never calls, so starting it doesn't map
// and initialise them (and the DLLs they pull in). Each loads on its first call instead:
// pdh and powrprof are sysinfo's, secur32 has only GetUserNameExW, shell32 only
// CommandLineToArgvW and oleaut32 only GetErrorInfo. All are system DLLs, so loading them
// later can't fail where loading them at start would have succeeded.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // the target's, not the host's: a build script runs on the host
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return;
    }
    // Only the core's own binary: the dashboard links the library into an app of its own.
    for dll in ["shell32", "oleaut32", "pdh", "powrprof", "secur32"] {
        println!("cargo:rustc-link-arg-bins=/DELAYLOAD:{dll}.dll");
    }
    // the helper that resolves delay-loaded imports on their first call
    println!("cargo:rustc-link-arg-bins=delayimp.lib");
}
