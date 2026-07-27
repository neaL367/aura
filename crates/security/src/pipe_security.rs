use std::alloc::{Layout, alloc_zeroed, dealloc};
use windows::Win32::Security::{self, ACCESS_ALLOWED_ACE, ACL, ACL_REVISION};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::Result;

const STANDARD_RIGHTS_GENERIC_EXECUTE: u32 = 0x20000000;
const FILE_GENERIC_WRITE: u32 = STANDARD_RIGHTS_GENERIC_EXECUTE | 0x4 | 0x80;
const FILE_GENERIC_READ: u32 = STANDARD_RIGHTS_GENERIC_EXECUTE | 0x1 | 0x80;

pub struct SecurityDescriptor {
    sd: *mut Security::SECURITY_DESCRIPTOR,
    sd_layout: Layout,
    acl_buf: Vec<u8>,
}

impl SecurityDescriptor {
    pub fn for_current_user() -> Result<Self> {
        let user_sid = get_current_user_sid()?;
        let acl_buf = create_restricted_acl(&user_sid)?;
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

fn create_restricted_acl(user_sid: &[u8]) -> Result<Vec<u8>> {
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

        let access_mask = FILE_GENERIC_READ | FILE_GENERIC_WRITE;

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
