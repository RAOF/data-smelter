#![feature(libc)]
#![feature(rustc_private)]

extern crate libc;
use libc::{c_int, c_ulong, c_void};

extern crate rustc_back;

use std::io::Error;
use std::result::Result;
use std::os::unix::io::AsRawFd;

#[link(name = "c")]
extern {
    fn ioctl(fd : c_int, request : c_ulong, ...) -> c_int;
}

const IOC_WRITE: i32 = 1;
const IOC_READ:  i32 = 2;

const IOC_NRBITS:   usize = 8;
const IOC_TYPEBITS: usize = 8;
const IOC_SIZEBITS: usize = 14;

const IOC_NRSHIFT:   usize = 0us;
const IOC_TYPESHIFT: usize = IOC_NRSHIFT   + IOC_NRBITS;
const IOC_SIZESHIFT: usize = IOC_TYPESHIFT + IOC_TYPEBITS;
const IOC_DIRSHIFT:  usize = IOC_SIZESHIFT + IOC_SIZEBITS;

#[inline]
pub fn ioc(dir: i32, ty: i32, nr: i32, size: i32) -> c_ulong {
    ((dir  << IOC_DIRSHIFT)  |
    (ty   << IOC_TYPESHIFT) |
    (nr   << IOC_NRSHIFT)   |
    (size << IOC_SIZESHIFT)) as c_ulong
}

fn get_hot_data(fd: c_int) -> Result<hot_data, Error> {
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
    unsafe {
        if ioctl(fd, ioc(IOC_READ, 'f' as i32, 17, std::mem::size_of::<hot_data>() as i32), &mut result) == 0
        {
            Ok(result)
        }
        else
        {
            Err(Error::last_os_error())
        }
    }
}

#[cfg(test)]
struct HotDataFixture {
    base_dir : std::path::PathBuf,
    fs_image_file : std::fs::File,
    fs_image_dir : std::path::PathBuf
}

#[cfg(test)]
impl Drop for HotDataFixture {
    fn drop(&mut self) {
        assert!(Command::new("sudo").arg("umount").arg(&self.base_dir.as_os_str()).status().is_ok());
        assert!(std::fs::remove_dir_all(&self.base_dir).is_ok());
        assert!(std::fs::remove_dir_all(&self.fs_image_dir).is_ok());
    }
}

#[cfg(test)]
use std::process::Command;

#[cfg(test)]
impl HotDataFixture {
    fn new(mkfs_command : &str) -> HotDataFixture {
        let tmpdir = rustc_back::tempdir::TempDir::new("hot-data-test").unwrap();
        let image_path = tmpdir.path().join("btrfs-image");
        let mount_dir = tmpdir.path().join("btrfs-mount-point");

        let image_file = std::fs::File::create(&image_path).unwrap();
        assert!(image_file.set_len(1024*1024*100).is_ok());
        assert!(image_file.sync_all().is_ok());

        assert!(std::fs::create_dir(&mount_dir).is_ok());

        assert!(Command::new(mkfs_command).arg(&image_path.as_os_str()).status().is_ok());
        assert!(Command::new("sudo").arg("mount").arg("-o").arg("hot_track").arg(&image_path.as_os_str()).arg(&mount_dir.as_os_str()).status().is_ok());
        assert!(Command::new("sudo").arg("chmod").arg("777").arg(&mount_dir.as_os_str()).status().is_ok());

        HotDataFixture { base_dir : mount_dir,
                         fs_image_file : image_file,
                         fs_image_dir : tmpdir.into_path() }
    }
}

#[test]
fn ioctl_binding_returns_error() {
    match get_hot_data(33) {
        Ok(_) => panic!("ioctl unexpectedly succeeded"),
        Err(e) => assert_eq!(e.raw_os_error().unwrap(), 9) // EBADF == 9
    }
}

#[test]
fn get_hot_data_returns_unsupported() {
    let test_file = std::fs::File::open("/home/chris/.zshrc").ok().unwrap();
    match get_hot_data(test_file.as_raw_fd()) {
        Ok(_) => panic!("Call unexpectedly succeeded"),
        Err(e) => assert_eq!(e.raw_os_error().unwrap(), 95) // ENOTSUP == 95
    }
}

#[test]
fn get_hot_data_returns_no_data() {
    let fixture = HotDataFixture::new("mkfs.btrfs");

    let test_file = match std::fs::File::create(&fixture.base_dir.join("test")) {
        Ok(file) => file,
        Err(e) => panic!("Failed to create {:?}: {}", &fixture.base_dir.join("test"), e)
    };
    match get_hot_data(test_file.as_raw_fd()) {
        Ok(_) => panic!("Call unexpectedly succeeded"),
        Err(e) => assert_eq!(e.raw_os_error().unwrap(), 61) // ENODATA == 61
    }
}

#[test]
fn get_hot_data_returns_fault() {
    let fixture = HotDataFixture::new("mkfs.btrfs");

    let test_file = match std::fs::File::create(&fixture.base_dir.join("test")) {
        Ok(file) => file,
        Err(e) => panic!("Failed to create {:?}: {}", &fixture.base_dir.join("test"), e)
    };

    unsafe {
        if ioctl(test_file.as_raw_fd(), ioc(IOC_READ, 'f' as i32, 17, std::mem::size_of::<hot_data>() as i32), 0) == 0
        {
            panic!("Call unexpectedly succeeded");
        }
        else
        {
            assert_eq!(Error::last_os_error().raw_os_error().unwrap(), 14) // EFAULT == 14
        }
    }
}

#[cfg(test)]
use std::io::Write;
#[cfg(test)]
use std::io::Read;

#[test]
fn get_hot_data_has_correct_read_write_count() {
    let fixture = HotDataFixture::new("mkfs.xfs");
    let test_file_name = fixture.base_dir.join("test_file");

    {
        let mut test_file = match std::fs::File::create(&test_file_name) {
            Ok(file) => file,
            Err(e) => panic!("Failed to create {:?}: {}", &test_file_name, e)
        };
        match test_file.write_all(b"Hello, I'm a string") {
            Err(why) => panic!("Failed to write: {}", why),
            Ok(_) => {}
        }
        test_file.sync_all();
    }

    let mut hello = match std::fs::File::open(&test_file_name) {
        Ok(file) => file,
        Err(e) => panic!("Failed to open {:?}: {}", &test_file_name, e)
    };

    let mut data = String::new();
    match hello.read_to_string(&mut data) {
        Ok(_) => assert_eq!(data, "Hello, I'm a string"),
        Err(e) => panic!("Failed to read from {:?}: {}", &test_file_name, e)
    }

    let info = match get_hot_data(hello.as_raw_fd()) {
        Ok(x) => x,
        Err(why) => panic!("get_hot_data failed: {}", why)
    };

    assert_eq!(info.num_reads, 1);
    assert_eq!(info.num_writes, 1);
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
