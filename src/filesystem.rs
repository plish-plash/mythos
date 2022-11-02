use fat32::volume::Volume;
use uniquelock::UniqueOnce;

pub type File = fat32::file::File<'static, ata::Partition>;

static USER_FILESYSTEM: UniqueOnce<Volume<ata::Partition>> = UniqueOnce::new();

pub fn init_fs(user_partition: ata::Partition) {
    USER_FILESYSTEM
        .call_once(|| Volume::new(user_partition))
        .expect("init_fs called twice");
    // TODO print some info about the filesystem
}

pub fn get_filesystem() -> Option<&'static Volume<ata::Partition>> {
    USER_FILESYSTEM.get().ok()
}
