use wdk_sys::ntddk::DbgBreakPointWithStatus;
#[macro_export]
macro_rules! kernel_callback {
    (fn $fn_name:ident( $($params:tt)* ) -> $ret_type:ty {
        $($body:tt)*
    }) => {
        #[link_section = "PAGE"]
        pub unsafe extern "C" fn $fn_name( $($params)* ) -> $ret_type {
            wdk::paged_code!();
            $($body)*
        }
    };
}

#[macro_export]
macro_rules! driver_entry { (($driver:ident, $registry_path:ident) { $($body:tt)* }) => {
        #[link_section = "INIT"]
        #[export_name = "DriverEntry"] // WDF expects a symbol with the name DriverEntry
        extern "system" fn driver_entry(
            $driver: &mut wdk_sys::DRIVER_OBJECT,
            $registry_path: wdk_sys::PCUNICODE_STRING,
        ) -> wdk_sys::NTSTATUS {
            $($body)*
        }
    };
}


#[macro_export]
macro_rules! init_object {
    ($type:ty, { $($field:ident : $value:expr),* $(,)* }) => {{
        let mut object = <$type>::default();
        $(object.$field = $value;)*
        object.Size = core::mem::size_of::<$type>() as ULONG;
        object
    }};
}

pub const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
    (device_type << 16) | (access << 14) | (function << 2) | method
}

pub const DEBUG: bool = true;

#[macro_export]
macro_rules! dbg {
    // NOTE: We cannot use `concat!` to make a static string as a format argument
    // of `eprintln!` because `file!` could contain a `{` or
    // `$val` expression could be a block (`{ .. }`), in which case the `eprintln!`
    // will be malformed.
    () => {
        $crate::debug_print!("[WAWAWA] [{}:{}]", core::file!(), core::line!())
    };
    ($val:expr $(,)?) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                $crate::debug_print!("[WAWAWA] [{}:{}] {} = {:#?}",
                    core::file!(), core::line!(), core::stringify!($val), &tmp);
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg!($val)),+,)
    };
}


#[macro_export]
macro_rules! debug_print {
    ($($arg:tt)*) => {
        if $crate::utils::DEBUG {
            wdk::println!($($arg)*);
        }
    };
}
fn breakpoint() {
    if DEBUG {
        debug_print!("Breakpoint");
        unsafe { DbgBreakPointWithStatus(0) };
    }
}
