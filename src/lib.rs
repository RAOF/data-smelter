#![feature(libc)]

extern crate libc;
use libc::{c_int, c_ulong};

use std::io::Error;

#[link(name = "c")]
extern {
    fn ioctl(fd : c_int, request : c_ulong, ...) -> c_int;
}

#[test]
fn ioctl_binding_returns_error() {
    unsafe {
        let mut result = hot_data { live: 1,
                                    resv: [0,0,0],
                                    temp: 0,
                                    avg_delta_reads: 0,
                                    avg_delta_writes: 0,
                                    last_read_time: 0,
                                    last_write_time: 0,
                                    num_reads: 0,
                                    num_writes: 0,
                                    future: [0,0,0,0]};
        let ret = ioctl(33, 1, (&mut result as *mut _ as *mut c_void));
        assert!(ret != 0);
        // EBADF == 9
        assert_eq!(Error::last_os_error().raw_os_error().unwrap(), 9);
    }
}

#[repr(C)]
struct hot_data {
    live : u8,
    resv : [u8; 3],
    temp : u32,
    avg_delta_reads : u64,
    avg_delta_writes : u64,
    last_read_time : u64,
    last_write_time : u64,
    num_reads : u32,
    num_writes : u32,
    future : [u64; 4]
}
