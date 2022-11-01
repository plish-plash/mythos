use fat32::volume::Volume;

pub type File = fat32::file::File<'static, ata::Partition>;

static USER_FILESYSTEM: spin::Once<Volume<ata::Partition>> = spin::Once::new();

pub fn init(user_partition: ata::Partition) {
    USER_FILESYSTEM.call_once(|| Volume::new(user_partition));
    // TODO print some info about the filesystem
}

pub fn get_filesystem() -> Option<&'static Volume<ata::Partition>> {
    USER_FILESYSTEM.get()
}
