use wdk_sys::ntddk::DbgBreakPointWithStatus;
#[macro_export]
macro_rules! kernel_callback {
    (fn $fn_name:ident($($param_name:ident: $param_type:ty),*) -> $ret_type:ty {
        $($body:tt)*
    }) => {
        #[link_section = "PAGE"]
        $vis unsafe extern "C" fn $fn_name($($param_name: $param_type),*) -> $ret_type {
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


const DEBUG: bool = true;
macro_rules! dbg {
    // NOTE: We cannot use `concat!` to make a static string as a format argument
    // of `eprintln!` because `file!` could contain a `{` or
    // `$val` expression could be a block (`{ .. }`), in which case the `eprintln!`
    // will be malformed.
    () => {
        $crate::debug_print!("[{}:{}]", $crate::file!(), $crate::line!())
    };
    ($val:expr $(,)?) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                $crate::debug_print!("[{}:{}] {} = {:#?}",
                    $crate::file!(), $crate::line!(), $crate::stringify!($val), &tmp);
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
        if DEBUG {
            wdk::println!($($arg)*);
        }
    };
}
fn breakpoint() {
    if DEBUG {
        unsafe {
            debug_print!("Breakpoint");
            unsafe { DbgBreakPointWithStatus(0) };
        }
    }
}
