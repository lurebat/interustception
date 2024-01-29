use core::result;
use snafu::Snafu;
use wdk_sys::{NT_SUCCESS, NTSTATUS, STATUS_SUCCESS};
use crate::dbg;


#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ErrorCode {
    DriverEntryFailed,
    DeviceCreationFailed,
    QueueCreationFailed,
    PdoInitAssignRawDeviceFailed,
    PdoInitAssignDeviceIdFailed,
    PdoInitAssignInstanceIdFailed,
    PdoInitAddDeviceTextFailed,
    DeviceCreateDeviceInterfaceFailed,
    FdoAddStaticChildFailed,
}

#[derive(Snafu, Debug)]
pub enum Error {
    #[snafu(display("{:#?}: {nt_status:#010X}"))]
    NtStatusError {
        nt_status: NTSTATUS,
        error_code: ErrorCode,
    }
}

impl Error {
    pub fn nt_status(&self) -> NTSTATUS {
        match self {
            Error::NtStatusError { nt_status, .. } => *nt_status,
        }
    }
}

pub(crate) trait ToStatus {
    fn to_status(self) -> NTSTATUS;
}

impl<T> ToStatus for Result<T> {
    fn to_status(self) -> NTSTATUS {
        match self {
            Ok(_) => STATUS_SUCCESS,
            Err(e) => e.nt_status(),
        }
    }
}

pub(crate) trait NtStatusError {
    fn check_status(self, error_code: ErrorCode) -> Result<()>;
}

impl NtStatusError for NTSTATUS {
    fn check_status(self, error_code: ErrorCode) -> Result<()> {
        if NT_SUCCESS(self) {
            Ok(())
        } else {
            Err(dbg!(Error::NtStatusError { error_code, nt_status: self }))
        }
    }
}

pub type Result<T> = result::Result<T, Error>;
