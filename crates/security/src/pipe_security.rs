use std::alloc::{Layout, alloc_zeroed, dealloc};
use windows::Win32::Security::{self, ACCESS_ALLOWED_ACE, ACL, ACL_REVISION};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::Result;

// Standard access rights
const SYNCHRONIZE: u32 = 0x0010_0000;
const READ_CONTROL: u32 = 0x0002_0000;

// File object access rights
const FILE_READ_DATA: u32 = 0x0000_0001;
const FILE_WRITE_DATA: u32 = 0x0000_0002;
const FILE_APPEND_DATA: u32 = 0x0000_0004;
const FILE_READ_EA: u32 = 0x0000_0008;
const FILE_WRITE_EA: u32 = 0x0000_0010;
const FILE_EXECUTE: u32 = 0x0000_0020;
const FILE_READ_ATTRIBUTES: u32 = 0x0000_0080;
const FILE_WRITE_ATTRIBUTES: u32 = 0x0000_0100;

// Computed file generic access masks (matching Win32 FILE_GENERIC_READ/WRITE)
pub const FILE_GENERIC_READ: u32 = FILE_READ_DATA | FILE_READ_ATTRIBUTES | FILE_READ_EA | SYNCHRONIZE | READ_CONTROL;
pub const FILE_GENERIC_WRITE: u32 = FILE_WRITE_DATA | FILE_WRITE_ATTRIBUTES | FILE_WRITE_EA | FILE_APPEND_DATA | SYNCHRONIZE | READ_CONTROL;

// Mutex object access: DELETE | READ_CONTROL | WRITE_DAC | WRITE_OWNER | SYNCHRONIZE | MUTEX_MODIFY_STATE
const MUTEX_ALL_ACCESS: u32 = 0x001F_0001;

pub struct SecurityDescriptor {
    sd: *mut Security::SECURITY_DESCRIPTOR,
    sd_layout: Layout,
    acl_buf: Vec<u8>,
}

impl SecurityDescriptor {
    /// Create a descriptor that grants the current user all access to the object.
    /// Suitable for mutex kernel objects (singleton).
    pub fn for_current_user() -> Result<Self> {
        Self::for_current_user_with_access(MUTEX_ALL_ACCESS)
    }

    /// Create a descriptor that grants the current user the specified access mask.
    pub fn for_current_user_with_access(access_mask: u32) -> Result<Self> {
        let user_sid = get_current_user_sid()?;
        let acl_buf = create_restricted_acl(&user_sid, access_mask)?;
        let (sd, sd_layout) = create_security_descriptor(&acl_buf)?;

        Ok(Self {
            sd,
            sd_layout,
            acl_buf,
        })
    }

    pub fn security_descriptor_ptr(&self) -> *mut core::ffi::c_void {
        self.sd as *mut core::ffi::c_void
    }

    pub fn acl_ptr(&self) -> *const ACL {
        self.acl_buf.as_ptr() as *const ACL
    }

    pub fn as_raw_security_attributes(&self) -> Security::SECURITY_ATTRIBUTES {
        Security::SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<Security::SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.sd as *mut core::ffi::c_void,
            bInheritHandle: false.into(),
        }
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.sd as *mut u8, self.sd_layout);
        }
    }
}

unsafe impl Send for SecurityDescriptor {}
unsafe impl Sync for SecurityDescriptor {}

fn get_current_user_sid() -> Result<Vec<u8>> {
    unsafe {
        let mut token = Default::default();
        OpenProcessToken(GetCurrentProcess(), Security::TOKEN_QUERY, &mut token)?;

        let mut len = 0u32;
        let _ = Security::GetTokenInformation(token, Security::TokenUser, None, 0, &mut len);

        let mut buf = vec![0u8; len as usize];
        Security::GetTokenInformation(
            token,
            Security::TokenUser,
            Some(buf.as_mut_ptr() as *mut _),
            len,
            &mut len,
        )?;

        let token_user = &*(buf.as_ptr() as *const Security::TOKEN_USER);
        let sid_len = Security::GetLengthSid(token_user.User.Sid);
        let mut sid = vec![0u8; sid_len as usize];
        Security::CopySid(
            sid_len,
            Security::PSID(sid.as_mut_ptr() as *mut _),
            token_user.User.Sid,
        )?;

        Ok(sid)
    }
}

fn create_restricted_acl(user_sid: &[u8], access_mask: u32) -> Result<Vec<u8>> {
    unsafe {
        let sid_ptr = Security::PSID(user_sid.as_ptr() as *mut _);

        let ace_size = std::mem::size_of::<ACCESS_ALLOWED_ACE>() as u32
            + Security::GetLengthSid(sid_ptr)
            - std::mem::size_of::<u32>() as u32;

        let acl_header_size = std::mem::size_of::<ACL>() as u32;
        let acl_total_size = acl_header_size + ace_size;

        let mut acl_buf = vec![0u8; acl_total_size as usize];
        let acl = acl_buf.as_mut_ptr() as *mut ACL;

        Security::InitializeAcl(acl, acl_total_size, ACL_REVISION)?;

        Security::AddAccessAllowedAce(acl, ACL_REVISION, access_mask, sid_ptr)?;

        Ok(acl_buf)
    }
}

fn create_security_descriptor(
    acl_buf: &[u8],
) -> Result<(*mut Security::SECURITY_DESCRIPTOR, Layout)> {
    unsafe {
        let layout = Layout::new::<Security::SECURITY_DESCRIPTOR>();
        let sd = alloc_zeroed(layout) as *mut Security::SECURITY_DESCRIPTOR;

        let sd_ptr = windows::Win32::Security::PSECURITY_DESCRIPTOR(sd as *mut _);
        Security::InitializeSecurityDescriptor(sd_ptr, 1)?;

        let acl = acl_buf.as_ptr() as *const ACL;
        Security::SetSecurityDescriptorDacl(sd_ptr, true, Some(acl as *const _), false)?;

        Ok((sd, layout))
    }
}

#[cfg(target_os = "windows")]
#[link(name = "advapi32")]
unsafe extern "system" {
    fn SetSecurityInfo(
        handle: windows::Win32::Foundation::HANDLE,
        object_type: u32,
        security_info: u32,
        owner: *const core::ffi::c_void,
        group: *const core::ffi::c_void,
        dacl: *const core::ffi::c_void,
        sacl: *const core::ffi::c_void,
    ) -> u32;
}

// Apply a restrictive DACL to a named pipe using raw `SetSecurityInfo` from `advapi32`.
// (windows crate 0.62 does not export this function, so we link it directly.)
#[cfg(target_os = "windows")]
pub fn apply_pipe_dacl(pipe_handle: isize, sd: &SecurityDescriptor) {
    const SE_KERNEL_OBJECT: u32 = 6;
    const DACL_SECURITY_INFORMATION: u32 = 4;

    unsafe {
        let _ = SetSecurityInfo(
            windows::Win32::Foundation::HANDLE(pipe_handle as *mut _),
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null(),
            std::ptr::null(),
            sd.acl_ptr() as *const _,
            std::ptr::null(),
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub fn apply_pipe_dacl(_pipe_handle: isize, _sd: &SecurityDescriptor) {}

/// Get the process ID of a named pipe client.
#[cfg(target_os = "windows")]
pub fn get_named_pipe_client_pid(pipe_handle: isize) -> Result<u32> {
    unsafe {
        let mut pid = 0u32;
        windows::Win32::System::Pipes::GetNamedPipeClientProcessId(
            windows::Win32::Foundation::HANDLE(pipe_handle as *mut _),
            &mut pid,
        )?;
        Ok(pid)
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_named_pipe_client_pid(_pipe_handle: isize) -> Result<u32> {
    Ok(0)
}
