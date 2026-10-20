use std::os::windows::io::AsRawHandle;
use tokio::net::windows::named_pipe::NamedPipeClient;
use tokio::net::windows::named_pipe::NamedPipeServer;
use tokio::net::windows::named_pipe::ServerOptions;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::GetTokenInformation;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Security::TOKEN_QUERY;
use windows_sys::Win32::Security::TOKEN_USER;
use windows_sys::Win32::Security::TokenUser;
use windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId;
use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Threading::OpenProcess;
use windows_sys::Win32::System::Threading::OpenProcessToken;
use windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION;
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn process_sid(process: HANDLE) -> eyre::Result<String> {
    let mut token = std::ptr::null_mut();
    // SAFETY: live process handle and writable token output. The owned token is closed below.
    let opened = unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut token) };
    eyre::ensure!(opened != 0, "Watcher peer identity unavailable");
    let result = (|| {
        let mut needed = 0;
        // SAFETY: null buffer requests the required size only.
        unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &raw mut needed) };
        eyre::ensure!(
            (1..=65536).contains(&needed),
            "Watcher peer identity unavailable"
        );
        let mut data =
            vec![0usize; usize::try_from(needed)?.div_ceil(std::mem::size_of::<usize>())];
        // SAFETY: aligned allocation is at least the returned required size.
        let queried = unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                data.as_mut_ptr().cast(),
                needed,
                &raw mut needed,
            )
        };
        eyre::ensure!(queried != 0, "Watcher peer identity unavailable");
        // SAFETY: successful TokenUser response occupies this live aligned allocation.
        let user = unsafe { &*data.as_ptr().cast::<TOKEN_USER>() };
        let mut text = std::ptr::null_mut();
        // SAFETY: SID is live and output receives a LocalAlloc NUL-terminated string.
        let converted = unsafe { ConvertSidToStringSidW(user.User.Sid, &raw mut text) };
        eyre::ensure!(converted != 0, "Watcher peer identity unavailable");
        let mut length = 0;
        loop {
            let current = text.wrapping_add(length);
            // SAFETY: successful conversion returned a live NUL-terminated SID
            // allocation. The loop stops at its terminator.
            if unsafe { *current } == 0 {
                break;
            }
            length += 1;
        }
        // SAFETY: measured UTF16 slice remains within the LocalAlloc string.
        let value = String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) });
        // SAFETY: this allocation came from ConvertSidToStringSidW.
        unsafe { LocalFree(text.cast()) };
        Ok(value?)
    })();
    // SAFETY: OpenProcessToken returned this owned handle and it is closed once.
    unsafe { CloseHandle(token) };
    result
}
pub(super) fn current_sid() -> eyre::Result<String> {
    // SAFETY: current process pseudo-handle needs no close.
    process_sid(unsafe { GetCurrentProcess() })
}
fn verify_pid(pid: u32) -> eyre::Result<()> {
    // SAFETY: query-only process open, with no inheritable handle.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    eyre::ensure!(!process.is_null(), "Watcher peer identity unavailable");
    let identity = process_sid(process);
    // SAFETY: OpenProcess returned this owned handle.
    unsafe { CloseHandle(process) };
    eyre::ensure!(
        identity? == current_sid()?,
        "Watcher IPC peer belongs to another user"
    );
    Ok(())
}
pub(super) fn verify_client(pipe: &NamedPipeServer) -> eyre::Result<()> {
    let mut pid = 0;
    // SAFETY: connected pipe owns a live handle and PID output is writable.
    let queried = unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle().cast(), &raw mut pid) };
    eyre::ensure!(queried != 0, "Watcher client identity unavailable");
    verify_pid(pid)
}
pub(super) fn verify_server(
    pipe: &NamedPipeClient,
) -> eyre::Result<super::daemon_peer_process::DaemonPeerProcess> {
    use std::os::windows::io::FromRawHandle;
    use std::os::windows::io::OwnedHandle;
    let mut pid = 0;
    // SAFETY: connected pipe owns a live handle and PID output is writable.
    let queried = unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle().cast(), &raw mut pid) };
    eyre::ensure!(queried != 0, "Watcher server identity unavailable");
    // SAFETY: query/synchronization-only access, no inheritable handle.
    let process = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION
                | windows_sys::Win32::System::Threading::PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    eyre::ensure!(!process.is_null(), "Watcher server identity unavailable");
    // SAFETY: OpenProcess returned an owned handle, transferred exactly once.
    let handle = unsafe { OwnedHandle::from_raw_handle(process.cast()) };
    eyre::ensure!(
        process_sid(process)? == current_sid()?,
        "Watcher IPC server belongs to another user"
    );
    Ok(super::daemon_peer_process::DaemonPeerProcess { handle })
}
pub(super) fn create(endpoint: &str, first: bool) -> eyre::Result<NamedPipeServer> {
    let sddl = wide(&format!("D:P(A;;GA;;;{})", current_sid()?));
    let mut descriptor = std::ptr::null_mut();
    // SAFETY: NUL-terminated SDDL and valid output pointer.
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &raw mut descriptor,
            std::ptr::null_mut(),
        )
    };
    eyre::ensure!(converted != 0, "Watcher pipe security unavailable");
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(std::mem::size_of::<SECURITY_ATTRIBUTES>())?,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    // SAFETY: descriptor/attributes outlive CreateNamedPipe, which copies the descriptor.
    let result = unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(endpoint, (&raw mut attributes).cast())
    };
    // SAFETY: allocation came from SDDL conversion and is released exactly once.
    unsafe { LocalFree(descriptor) };
    Ok(result?)
}
