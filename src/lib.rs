// Copyright (c) Microsoft Corporation.
// License: MIT OR Apache-2.0

//! # Abstract
//!
//!    This driver demonstrates use of a default I/O Queue, its
//!    request start events, cancellation event, and a synchronized DPC.
//!
//!    To demonstrate asynchronous operation, the I/O requests are not completed
//!    immediately, but stored in the drivers private data structure, and a
//!    timer DPC will complete it next time the DPC runs.
//!
//!    During the time the request is waiting for the DPC to run, it is
//!    made cancellable by the call WdfRequestMarkCancelable. This
//!    allows the test program to cancel the request and exit instantly.
//!
//!    This rather complicated set of events is designed to demonstrate
//!    the driver frameworks synchronization of access to a device driver
//!    data structure, and a pointer which can be a proxy for device hardware
//!    registers or resources.
//!
//!    This common data structure, or resource is accessed by new request
//!    events arriving, the DPC that completes it, and cancel processing.
//!
//!    Notice the lack of specific lock/unlock operations.
//!
//!    Even though this example utilizes a serial queue, a parallel queue
//!    would not need any additional explicit synchronization, just a
//!    strategy for managing multiple requests outstanding.

#![no_std]
#![cfg_attr(feature = "nightly", feature(hint_must_use))]
//#![deny(warnings)]
//#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
#![allow(clippy::missing_safety_doc)]

mod device;
mod driver;

#[cfg(not(test))]
extern crate wdk_panic;
extern crate alloc;

use core::ptr::null_mut;
#[cfg(not(test))]
use wdk_alloc::WDKAllocator;
use wdk_sys::{*, ntddk::KeGetCurrentIrql};

mod wdf_object_context;
mod foreign;

use wdf_object_context::{wdf_declare_context_type};
use crate::foreign::{ConnectData, KeyboardAttributes};
use crate::wdf_object_context::wdf_declare_context_type_with_name;

#[cfg(not(test))]
#[global_allocator]
static GLOBAL_ALLOCATOR: WDKAllocator = WDKAllocator;

// {CDC35B6E-0BE4-4936-BF5F-5537380A7C1A}
const GUID_DEVINTERFACE_ECHO: GUID = GUID {
    Data1: 0xCDC3_5B6Eu32,
    Data2: 0x0BE4u16,
    Data3: 0x4936u16,
    Data4: [
        0xBFu8, 0x5Fu8, 0x55u8, 0x37u8, 0x38u8, 0x0Au8, 0x7Cu8, 0x1Au8,
    ],
};



impl Default for ConnectData {
    fn default() -> Self {
        Self {
            class_device_object: null_mut(),
            class_service: null_mut(),
        }
    }
}

pub struct DeviceContext {
    raw_pdo_queue: WDFQUEUE,
    upper_connect_data: ConnectData,
    
    keyboard_attributes: KeyboardAttributes,
}
wdf_declare_context_type!(DeviceContext);

pub struct PdoContext {
    instance: u32,
    queue: WDFQUEUE
}

wdf_declare_context_type_with_name!(PdoContext, get_pdo_context);