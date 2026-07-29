//! 一次性探针：打印 sysinfo 磁盘 I/O 累计与增量字节。

use std::thread;
use std::time::Duration;

use sysinfo::Disks;

fn print_disks(disks: &Disks, label: &str) {
    println!("=== {} ===", label);
    for disk in disks.list() {
        let usage = disk.usage();
        println!(
            "{} @ {} | delta read={} write={} | total read={} write={}",
            disk.name().to_string_lossy(),
            disk.mount_point().display(),
            usage.read_bytes,
            usage.written_bytes,
            usage.total_read_bytes,
            usage.total_written_bytes,
        );
    }
}

fn main() {
    let mut disks = Disks::new_with_refreshed_list();
    print_disks(&disks, "initial");
    thread::sleep(Duration::from_secs(1));
    disks.refresh(false);
    print_disks(&disks, "after 1s refresh");
}
