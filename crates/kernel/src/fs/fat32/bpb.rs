use core::{mem::MaybeUninit, ptr::addr_of_mut};

/// 参考 https://elixir.bootlin.com/linux/v2.6.39.4/source/include/linux/msdos_fs.h#L105
pub struct BiosParameterBlock {
    pub system_id: [u8; 8], // 通常是 "MSWIN4.1"

    pub sector_size: u16,           // 每个逻辑删去的字节数，通常是 512
    pub sector_per_cluster: u8,     // 每簇的扇区数
    pub reserved_sector_count: u16, // 保留扇区数，FAT32 通常是 32
    pub fat_count: u8,              // FAT 的数量，FAT32 通常是 2
    pub _root_entry_count: u16,     // 根目录条目数，FAT32 必须是 0
    pub _sector_count: u16,         // FAT16 的扇区总数，FAT32 必须是 0
    pub _media: u8,                 // media code，通常是 0xF8，无视
    pub _fat_length: u16,           // FAT16 中每个 FAT 占用的扇区数，FAT32 必须是 0
    pub _sector_per_track: u16,     // 每磁道扇区数，无视
    pub _head_count: u16,           // 磁头数，无视
    pub _hidden_sector_count: u32,  // 隐含扇区数，无视
    pub total_sector_count: u32,    // 应该是 FAT32 的扇区总数

    // 下面的部分是 FAT32 专用的
    pub fat32_length: u32, // FAT32 中每个 FAT 占用的扇区数
    pub _ext_flags: u16,   // 通常是 0
    pub _version: u16,     // 通常（必须？）是 0
    pub root_cluster: u32, // 根目录起始簇，通常是 2
    pub info_sector: u16,  // 文件系统信息占用的扇区 id，通常是 1
    pub backup_boot: u16,  // 备份引导扇区 id，通常是 6
}

impl BiosParameterBlock {
    pub fn new(src: &[u8; 512]) -> Self {
        let mut ret = MaybeUninit::<BiosParameterBlock>::uninit();
        struct LittleEndianReader<'a> {
            src: &'a [u8; 512],
            offset: usize,
        }
        impl LittleEndianReader<'_> {
            fn read_byte_array<const N: usize>(&mut self) -> [u8; N] {
                self.offset += N;
                self.src[self.offset - N..self.offset].try_into().unwrap()
            }
            fn read_u8(&mut self) -> u8 {
                self.offset += 1;
                self.src[self.offset - 1]
            }
            fn read_u16(&mut self) -> u16 {
                self.offset += 2;
                u16::from_le_bytes(self.src[self.offset - 2..self.offset].try_into().unwrap())
            }
            fn read_u32(&mut self) -> u32 {
                self.offset += 4;
                u32::from_le_bytes(self.src[self.offset - 4..self.offset].try_into().unwrap())
            }
        }
        // 前三个字节是跳转代码，无视
        let mut reader = LittleEndianReader { src, offset: 3 };
        let ptr = ret.as_mut_ptr();
        unsafe {
            macro load($field:ident, $value:expr) {
                addr_of_mut!((*ptr).$field).write($value);
            }
            load!(system_id, reader.read_byte_array::<8>());
            load!(sector_size, reader.read_u16());
            load!(sector_per_cluster, reader.read_u8());
            load!(reserved_sector_count, reader.read_u16());
            load!(fat_count, reader.read_u8());
            load!(_root_entry_count, reader.read_u16());
            load!(_sector_count, reader.read_u16());
            load!(_media, reader.read_u8());
            load!(_fat_length, reader.read_u16());
            load!(_sector_per_track, reader.read_u16());
            load!(_head_count, reader.read_u16());
            load!(_hidden_sector_count, reader.read_u32());
            load!(total_sector_count, reader.read_u32());
            load!(fat32_length, reader.read_u32());
            load!(_ext_flags, reader.read_u16());
            load!(_version, reader.read_u16());
            load!(root_cluster, reader.read_u32());
            load!(info_sector, reader.read_u16());
            load!(backup_boot, reader.read_u16());
        }
        debug_assert_eq!(reader.offset, 0x34);
        let ret = unsafe { ret.assume_init() };
        debug!("{ret}");
        ret
    }
}

impl core::fmt::Display for BiosParameterBlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "BiosParameterBlock {{")?;
        write!(
            f,
            "system_id: {}, ",
            &core::str::from_utf8(&self.system_id).unwrap()
        )?;
        write!(f, "sector_size: {}, ", &self.sector_size)?;
        write!(f, "sector_per_cluster: {}, ", &self.sector_per_cluster)?;
        write!(
            f,
            "reserved_sector_count: {}, ",
            &self.reserved_sector_count
        )?;
        write!(f, "fat_count: {}, ", &self.fat_count)?;
        write!(f, "total_sector_count: {}, ", &self.total_sector_count)?;
        write!(f, "fat32_length: {}, ", &self.fat32_length)?;
        write!(f, "root_cluster: {}, ", &self.root_cluster)?;
        write!(f, "info_sector: {}, ", &self.info_sector)?;
        write!(f, "backup_boot: {}, ", &self.backup_boot)?;
        write!(f, "}}")
    }
}
