use std::sync::RwLock;

use fat32::{BiosParameterBlock, BlockDevice, DirEntryBuilder, DirEntryBuilderResult, FileAllocTable, SECTOR_SIZE};

const FAT_ENTRIES_PER_SECTOR: usize = SECTOR_SIZE / core::mem::size_of::<u32>();
const TOTAL_SECTORS: usize = 129;

struct MemBlockDevice {
    sectors: RwLock<Vec<[u8; SECTOR_SIZE]>>,
}

impl MemBlockDevice {
    fn new(mut sectors: Vec<[u8; SECTOR_SIZE]>) -> Self {
        if sectors.len() < TOTAL_SECTORS {
            sectors.resize(TOTAL_SECTORS, [0; SECTOR_SIZE]);
        }
        Self {
            sectors: RwLock::new(sectors),
        }
    }
}

impl BlockDevice for MemBlockDevice {
    fn read_block(&self, block_id: usize, buf: &mut [u8; SECTOR_SIZE]) {
        let sectors = self.sectors.read().unwrap();
        buf.copy_from_slice(&sectors[block_id]);
    }
}

fn default_bpb() -> BiosParameterBlock {
    BiosParameterBlock {
        system_id: *b"MSWIN4.1",
        sector_size: SECTOR_SIZE as u16,
        sector_per_cluster: 1,
        reserved_sector_count: 2,
        fat_count: 1,
        _root_entry_count: 0,
        _sector_count: 0,
        _media: 0xF8,
        _fat_length: 0,
        _sector_per_track: 0,
        _head_count: 0,
        _hidden_sector_count: 0,
        total_sector_count: TOTAL_SECTORS as u32,
        fat32_length: 1,
        _ext_flags: 0,
        _version: 0,
        root_cluster: 2,
        info_sector: 1,
        backup_boot: 6,
    }
}

fn fat_entries_to_sector(entries: &[u32; FAT_ENTRIES_PER_SECTOR]) -> [u8; SECTOR_SIZE] {
    let mut sector = [0u8; SECTOR_SIZE];
    for (i, entry) in entries.iter().enumerate() {
        let offset = i * 4;
        sector[offset..offset + 4].copy_from_slice(&entry.to_le_bytes());
    }
    sector
}

fn fsinfo_sector(free_count: u32, next_free: u32) -> [u8; SECTOR_SIZE] {
    let mut sector = [0u8; SECTOR_SIZE];
    sector[0..4].copy_from_slice(&0x4161_5252u32.to_le_bytes());
    sector[484..488].copy_from_slice(&0x6141_7272u32.to_le_bytes());
    sector[488..492].copy_from_slice(&free_count.to_le_bytes());
    sector[492..496].copy_from_slice(&next_free.to_le_bytes());
    sector[508..512].copy_from_slice(&0xaa55_0000u32.to_le_bytes());
    sector
}

fn make_device(fat_entries: &[u32; FAT_ENTRIES_PER_SECTOR], fsinfo: [u8; SECTOR_SIZE]) -> &'static MemBlockDevice {
    let mut sectors = vec![[0u8; SECTOR_SIZE]; TOTAL_SECTORS];
    sectors[1] = fsinfo;
    sectors[2] = fat_entries_to_sector(fat_entries);
    Box::leak(Box::new(MemBlockDevice::new(sectors)))
}

fn base_fat_entries() -> [u32; FAT_ENTRIES_PER_SECTOR] {
    let mut entries = [0u32; FAT_ENTRIES_PER_SECTOR];
    entries[0] = 0x0fff_fff8;
    entries[1] = 0x0fff_ffff;
    entries
}

fn short_entry(name: [u8; 11], attr: u8) -> [u8; 32] {
    let mut entry = [0u8; 32];
    entry[0..11].copy_from_slice(&name);
    entry[11] = attr;
    entry
}

fn lfn_checksum(short_name: &[u8; 11]) -> u8 {
    let mut checksum = 0u8;
    for &byte in short_name {
        checksum = (checksum >> 1) + ((checksum & 1) << 7);
        checksum = checksum.wrapping_add(byte);
    }
    checksum
}

fn lfn_entry(order: u8, checksum: u8, fst_clus_lo: [u8; 2], name_part: [u16; 13]) -> [u8; 32] {
    let mut entry = [0u8; 32];
    entry[0] = order;
    entry[11] = 0x0F;
    entry[12] = 0;
    entry[13] = checksum;
    entry[26] = fst_clus_lo[0];
    entry[27] = fst_clus_lo[1];

    let mut i = 0;
    for offset in (1..11).step_by(2) {
        entry[offset..offset + 2].copy_from_slice(&name_part[i].to_le_bytes());
        i += 1;
    }
    for offset in (14..26).step_by(2) {
        entry[offset..offset + 2].copy_from_slice(&name_part[i].to_le_bytes());
        i += 1;
    }
    for offset in (28..32).step_by(2) {
        entry[offset..offset + 2].copy_from_slice(&name_part[i].to_le_bytes());
        i += 1;
    }
    entry
}

fn fat32_boot_sector_with_signature(valid_signature: bool) -> [u8; 512] {
    let mut bs = [0u8; 512];
    bs[0] = 0xEB;
    bs[1] = 0x58;
    bs[2] = 0x90;
    bs[3..11].copy_from_slice(b"MSWIN4.1");
    bs[11..13].copy_from_slice(&(SECTOR_SIZE as u16).to_le_bytes());
    bs[13] = 1;
    bs[14..16].copy_from_slice(&32u16.to_le_bytes());
    bs[16] = 2;
    bs[17..19].copy_from_slice(&0u16.to_le_bytes());
    bs[19..21].copy_from_slice(&0u16.to_le_bytes());
    bs[21] = 0xF8;
    bs[22..24].copy_from_slice(&0u16.to_le_bytes());
    bs[24..26].copy_from_slice(&0u16.to_le_bytes());
    bs[26..28].copy_from_slice(&0u16.to_le_bytes());
    bs[28..32].copy_from_slice(&0u32.to_le_bytes());
    bs[32..36].copy_from_slice(&(TOTAL_SECTORS as u32).to_le_bytes());
    bs[36..40].copy_from_slice(&1u32.to_le_bytes());
    bs[40..42].copy_from_slice(&0u16.to_le_bytes());
    bs[42..44].copy_from_slice(&0u16.to_le_bytes());
    bs[44..48].copy_from_slice(&2u32.to_le_bytes());
    bs[48..50].copy_from_slice(&1u16.to_le_bytes());
    bs[50..52].copy_from_slice(&6u16.to_le_bytes());
    if valid_signature {
        bs[510] = 0x55;
        bs[511] = 0xAA;
    }
    bs
}

#[test]
fn alloc_cluster_should_not_panic_when_no_free_cluster() {
    let mut entries = base_fat_entries();
    for entry in entries.iter_mut().skip(2) {
        *entry = 0x0fff_ffff;
    }
    let device = make_device(&entries, fsinfo_sector(0xFFFF_FFFF, 0xFFFF_FFFF));
    let fat = FileAllocTable::new(device, &default_bpb()).expect("fat init should succeed");

    let result = fat.alloc_cluster(None);
    assert!(result.is_none(), "alloc_cluster should return None instead of panic");
}

// 测试 FAT32 簇链遍历需要屏蔽高 4 位保留位，只使用低 28 位。
#[test]
fn cluster_chain_should_mask_high_4_bits() {
    let mut entries = base_fat_entries();
    entries[2] = 0xF000_0003;
    entries[3] = 0x0FFF_FFFF;
    let device = make_device(&entries, fsinfo_sector(0xFFFF_FFFF, 0xFFFF_FFFF));
    let fat = FileAllocTable::new(device, &default_bpb()).expect("fat init should succeed");

    let chain = fat.cluster_chain(2).collect::<Vec<_>>();
    assert_eq!(chain, vec![2, 3], "fat32 should only use lower 28 bits");
}

// 测试短文件名应按 8.3 规则解析为带点格式，空格是填充项，不被认为是文件名的一部分
#[test]
fn short_name_should_follow_8_3_format() {
    let entry = short_entry(*b"DOG     JPG", 0x20);
    let dir = match DirEntryBuilder::from_entry(&entry).expect("parse should succeed") {
        DirEntryBuilderResult::Final(dir) => dir,
        DirEntryBuilderResult::Builder(_) => panic!("expected standard entry"),
    };
    assert_eq!(dir.name(), "DOG.JPG");
}

// 测试分配结果需要持久化；重新挂载后不应重复分配同一簇。
// TODO: 暂时不实现持久化
// #[test]
#[expect(unused, reason = "暂时不实现持久化")]
fn alloc_should_be_persisted_after_remount() {
    let mut entries = base_fat_entries();
    entries[2] = 0;
    entries[3] = 0;
    let device = make_device(&entries, fsinfo_sector(2, 2));
    let bpb = default_bpb();

    let fat1 = FileAllocTable::new(device, &bpb).expect("first mount should succeed");
    let first = fat1.alloc_cluster(None).expect("first alloc should succeed");
    drop(fat1);

    let fat2 = FileAllocTable::new(device, &bpb).expect("second mount should succeed");
    let second = fat2.alloc_cluster(None).expect("second alloc should succeed");
    assert_ne!(first, second, "allocated cluster should have been written back to disk");
}

// 测试 FSInfo 无效时应回退扫描 FAT，而不是直接挂载失败。
#[test]
fn invalid_fsinfo_should_not_abort_mount() {
    let mut entries = base_fat_entries();
    entries[2] = 0;
    let invalid_fsinfo = [0u8; SECTOR_SIZE];
    let device = make_device(&entries, invalid_fsinfo);

    let result = FileAllocTable::new(device, &default_bpb());
    assert!(result.is_ok(), "invalid fsinfo should trigger FAT scan fallback");
}

// 测试目录判断应按 DIRECTORY 位判定，而非要求属性完全相等。
#[test]
fn is_dir_should_check_directory_bit() {
    let entry = short_entry(*b"SUBDIR     ", 0x10 | 0x20); // ATTR_DIRECTORY | ATTR_ARCHIVE
    let dir = match DirEntryBuilder::from_entry(&entry).expect("parse should succeed") {
        DirEntryBuilderResult::Final(dir) => dir,
        DirEntryBuilderResult::Builder(_) => panic!("expected standard entry"),
    };
    assert!(dir.is_dir(), "directory attribute can coexist with other flags");
}

// 测试 LFN 判定不能用 contains，非法属性组合不应被当作 LFN。
#[test]
fn lfn_detection_should_not_use_plain_contains() {
    let mut entry = [0u8; 32];
    entry[0] = 0x41;
    entry[11] = 0x1F;
    let parsed = DirEntryBuilder::from_entry(&entry);
    assert!(
        !matches!(parsed, Ok(DirEntryBuilderResult::Builder(_))),
        "attr 0x1F should not be accepted as LFN entry"
    );
}

// 测试 LFN 条目的 LDIR_FstClusLO 两个字节都必须为 0。
#[test]
fn lfn_fstcluslo_high_byte_must_be_zero() {
    let short = short_entry(*b"FILE    TXT", 0x20);
    let checksum = lfn_checksum(short[0..11].try_into().unwrap());
    let mut name_part = [0xFFFFu16; 13];
    name_part[0] = b'F' as u16;
    name_part[1] = 0;
    let lfn = lfn_entry(0x41, checksum, [0, 1], name_part);

    let result = DirEntryBuilder::from_entry(&lfn);
    assert!(
        result.is_err(),
        "lfn entry with non-zero LDIR_FstClusLO must be rejected"
    );
}

// 测试 BPB 解析必须校验引导扇区签名 0xAA55。
#[test]
fn bpb_parser_should_reject_invalid_boot_signature() {
    let bs = fat32_boot_sector_with_signature(false);
    let parsed = BiosParameterBlock::new(&bs);
    assert!(parsed.is_err(), "boot signature 0xAA55 is mandatory");
}
