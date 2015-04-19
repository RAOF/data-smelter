#![feature(libc)]
#![feature(rustc_private)]

extern crate libc;
use libc::{c_int, c_ulong, c_void};

extern crate rustc_back;

use std::io::Error;

#[link(name = "c")]
extern {
    fn ioctl(fd : c_int, request : c_ulong, ...) -> c_int;
}

#[cfg(test)]
struct HotDataSupportingFixture {
    base_dir : std::path::PathBuf,
    fs_image_file : std::fs::File,
    fs_image_dir : std::path::PathBuf
}

#[cfg(test)]
impl Drop for HotDataSupportingFixture {
    fn drop(&mut self) {
        assert!(Command::new("sudo").arg("umount").arg(&self.base_dir.as_os_str()).status().is_ok());
        assert!(std::fs::remove_dir_all(&self.base_dir).is_ok());
        assert!(std::fs::remove_dir_all(&self.fs_image_dir).is_ok());
    }
}

use std::process::Command;

#[cfg(test)]
impl HotDataSupportingFixture {
    fn new() -> HotDataSupportingFixture {
        let tmpdir = rustc_back::tempdir::TempDir::new("hot-data-test").unwrap();
        let image_path = tmpdir.path().join("btrfs-image");
        let mount_dir = tmpdir.path().join("btrfs-mount-point");

        let image_file = std::fs::File::create(&image_path).unwrap();
        assert!(image_file.set_len(1024*1024*100).is_ok());
        assert!(image_file.sync_all().is_ok());

        assert!(std::fs::create_dir(&mount_dir).is_ok());

        assert!(Command::new("mkfs.btrfs").arg(&image_path.as_os_str()).status().is_ok());
        assert!(Command::new("sudo").arg("mount").arg(&image_path.as_os_str()).arg(&mount_dir.as_os_str()).status().is_ok());

        HotDataSupportingFixture { base_dir : mount_dir,
                                   fs_image_file : image_file,
                                   fs_image_dir : tmpdir.into_path() }
    }
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
