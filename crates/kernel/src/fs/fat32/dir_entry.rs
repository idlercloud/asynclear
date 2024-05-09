use core::mem::MaybeUninit;

use bitflags::bitflags;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use compact_str::CompactString;
use defines::{
    error::{errno, KResult},
    misc::TimeSpec,
};
use smallvec::SmallVec;

const SNAME_MAX_LEN: usize = 11;
// 每个 lfn entry 中存放的 utf16 数量
const LNAME_PART_LEN: usize = 13;

pub(super) const DIR_ENTRY_SIZE: usize = 32;

pub struct DirEntry {
    pub(super) short_name: CompactString,
    pub(super) long_name: CompactString,
    attr: DirEntryAttr,
    create_date: u16,
    create_time: u16,
    create_ten_ms: u8,
    modify_date: u16,
    modify_time: u16,
    access_date: u16,
    first_cluster_id: u32,
    file_size: u32,
}

impl DirEntry {
    /// 优先用长文件名，如为空则用短文件名
    pub fn name(&self) -> &str {
        if !self.long_name.is_empty() {
            &self.long_name
        } else {
            &self.short_name
        }
    }

    pub fn is_dir(&self) -> bool {
        self.attr == DirEntryAttr::DIRECTORY
    }

    pub fn file_size(&self) -> usize {
        self.file_size as usize
    }

    pub fn take_name(&mut self) -> CompactString {
        if !self.long_name.is_empty() {
            core::mem::take(&mut self.long_name)
        } else {
            core::mem::take(&mut self.short_name)
        }
    }

    pub fn first_cluster_id(&self) -> u32 {
        self.first_cluster_id
    }

    pub fn create_time(&self) -> TimeSpec {
        let date = fat_date_to_naive_date(self.create_date);
        let time = fat_time_to_naive_time(self.create_time, self.create_ten_ms);
        let date_time = NaiveDateTime::new(date, time).and_utc();
        let sec = date_time.timestamp();
        let nsec = date_time.timestamp_subsec_nanos() as i64;
        TimeSpec { sec, nsec }
    }

    pub fn modify_time(&self) -> TimeSpec {
        let date = fat_date_to_naive_date(self.modify_date);
        let time = fat_time_to_naive_time(self.modify_time, 0);
        let date_time = NaiveDateTime::new(date, time).and_utc();
        let sec = date_time.timestamp();
        let nsec = date_time.timestamp_subsec_nanos() as i64;
        TimeSpec { sec, nsec }
    }

    pub fn access_time(&self) -> TimeSpec {
        let date = fat_date_to_naive_date(self.access_date);
        let time = NaiveTime::default();
        let date_time = NaiveDateTime::new(date, time).and_utc();
        let sec = date_time.timestamp();
        let nsec = date_time.timestamp_subsec_nanos() as i64;
        TimeSpec { sec, nsec }
    }
}

fn fat_date_to_naive_date(date: u16) -> NaiveDate {
    let year = (1980 + (date >> 9)) as i32;
    let month = (((date >> 5) & 0x0F) - 1) as u32;
    let day = ((date & 0x1F) - 1) as u32;
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn fat_time_to_naive_time(time: u16, ten_ms: u8) -> NaiveTime {
    let hour = ((time >> 11) & 0x1F) as u32;
    let min = ((time >> 5) & 0x3F) as u32;
    let sec = (time & 0x1F) as u32 * 2 + (ten_ms / 100) as u32;
    let ms = (ten_ms % 100) as u32 * 10;
    NaiveTime::from_hms_milli_opt(hour, min, sec, ms).unwrap()
}

bitflags! {
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DirEntryAttr: u8 {
        const READ_ONLY = 0x01;
        const HIDDEN    = 0x02;
        const SYSTEM    = 0x04;
        const VOLUME_ID = 0x08;
        const DIRECTORY = 0x10;
        const ARCHIVE   = 0x20;

        const LFN       = Self::READ_ONLY.bits() | Self::HIDDEN.bits() | Self::SYSTEM.bits() | Self::VOLUME_ID.bits();
    }
}

pub enum DirEntryBuilderResult {
    Builder(DirEntryBuilder),
    Final(DirEntry),
}

pub struct DirEntryBuilder {
    curr_order: u8,
    checksum: u8,
    name: SmallVec<[u16; 16]>,
}

impl DirEntryBuilder {
    pub fn from_entry(entry: &[u8; DIR_ENTRY_SIZE]) -> KResult<DirEntryBuilderResult> {
        let Some(attr) = DirEntryAttr::from_bits(entry[11]) else {
            warn!("dir entry attr is invalid: {}", entry[11]);
            return Err(errno::EINVAL);
        };
        if attr.contains(DirEntryAttr::LFN) {
            let mut builder = Self {
                curr_order: 0,
                checksum: 0,
                name: SmallVec::new(),
            };
            builder.read_lfn_entry(entry)?;

            Ok(DirEntryBuilderResult::Builder(builder))
        } else {
            read_standard_entry(entry).map(DirEntryBuilderResult::Final)
        }
    }

    fn read_lfn_entry(&mut self, entry: &[u8; DIR_ENTRY_SIZE]) -> KResult<()> {
        let curr_order = entry[0] & 0b01_1111;
        debug_assert_ne!(curr_order, 0);
        let lname_offset = (curr_order as usize - 1) * LNAME_PART_LEN;
        // lfn entry 应该是倒序存储的，curr_order 递减
        // 不过如果是倒序，那记录 final part 还有什么用（）
        let is_first_lfn_entry = self.curr_order == 0;
        if self.curr_order == 0 {
            self.name.resize(lname_offset + LNAME_PART_LEN, 0);
        } else if curr_order != self.curr_order - 1 {
            warn!(
                "lfn order is wrong, expected {}, found {curr_order}",
                self.curr_order - 1
            );
            return Err(errno::EINVAL);
        }
        self.curr_order = curr_order;

        let is_final_lfn_entry = entry[0] & (1 << 6) != 0;
        if is_first_lfn_entry && !is_final_lfn_entry {
            warn!("first read lfn entry should be marked as final");
            return Err(errno::EINVAL);
        }
        if !is_first_lfn_entry && is_final_lfn_entry {
            warn!("non-first read lfn entry should not be marked as final");
            return Err(errno::EINVAL);
        }

        // 12: Long entry type，对 fat32 的 lfn 来说应该是 0？
        // 26: Starting cluster，对 lfn 应始终为 0
        if entry[12] != 0 || entry[26] != 0 {
            warn!(
                "Should be zero. long entry type: {} starting cluster: {}",
                entry[12], entry[26]
            );
            return Err(errno::EINVAL);
        }
        let checksum = entry[13];
        // 多个 lfn 之间 checksum 应该是一致的？
        if is_first_lfn_entry {
            self.checksum = checksum;
        } else if self.checksum != checksum {
            warn!(
                "checksum incoherent for lfn entries: expected {}, found {checksum}",
                self.checksum
            );
            return Err(errno::EINVAL);
        };

        self.name[lname_offset..lname_offset + LNAME_PART_LEN].copy_from_slice(&lfn_part(entry));
        Ok(())
    }

    pub fn add_entry(mut self, entry: &[u8; DIR_ENTRY_SIZE]) -> KResult<DirEntryBuilderResult> {
        let Some(attr) = DirEntryAttr::from_bits(entry[11]) else {
            warn!("dir entry attr is invalid: {}", entry[11]);
            return Err(errno::EINVAL);
        };
        if attr.contains(DirEntryAttr::LFN) {
            self.read_lfn_entry(entry)?;
            Ok(DirEntryBuilderResult::Builder(self))
        } else {
            if self.curr_order != 1 {
                warn!("read standard entry withou order 1 lfn entry");
                return Err(errno::EINVAL);
            }
            let mut dir_entry = read_standard_entry(entry)?;
            let checksum = calc_checksum(entry[0..11].try_into().unwrap());
            if self.checksum != checksum {
                warn!(
                    "checksum incoherent for lfn and standard entries: expected {}, found {checksum}",
                    self.checksum
                );
                return Err(errno::EINVAL);
            }
            let mut lfn_len = 0;
            while lfn_len < self.name.len() && self.name[lfn_len] != 0 {
                lfn_len += 1;
            }
            dir_entry.long_name =
                CompactString::from_utf16(&self.name[..lfn_len]).map_err(|e| {
                    warn!("Invalid utf16 {:?}. {e}", &self.name[..lfn_len]);
                    errno::EINVAL
                })?;
            Ok(DirEntryBuilderResult::Final(dir_entry))
        }
    }
}

fn lfn_part(entry: &[u8; DIR_ENTRY_SIZE]) -> [u16; LNAME_PART_LEN] {
    let mut array = MaybeUninit::uninit_array();
    let mut i = 0;
    for &ucs2 in entry[1..1 + 10].array_chunks::<2>() {
        array[i] = MaybeUninit::new(u16::from_le_bytes(ucs2));
        i += 1;
    }
    for &ucs2 in entry[14..14 + 12].array_chunks::<2>() {
        array[i] = MaybeUninit::new(u16::from_le_bytes(ucs2));
        i += 1;
    }
    for &ucs2 in entry[28..28 + 4].array_chunks::<2>() {
        array[i] = MaybeUninit::new(u16::from_le_bytes(ucs2));
        i += 1;
    }
    unsafe { MaybeUninit::array_assume_init(array) }
}

fn read_standard_entry(entry: &[u8; DIR_ENTRY_SIZE]) -> KResult<DirEntry> {
    let mut short_name_len = 0;
    while short_name_len < SNAME_MAX_LEN && entry[short_name_len] != 0 {
        short_name_len += 1;
    }
    let Ok(short_name) = CompactString::from_utf8(&entry[..short_name_len]) else {
        warn!(
            "short name is not valid utf8: {:?}",
            &entry[..short_name_len]
        );
        return Err(errno::EINVAL);
    };
    let Some(attr) = DirEntryAttr::from_bits(entry[11]) else {
        warn!("dir entry attr is invalid: {}", entry[11]);
        return Err(errno::EINVAL);
    };
    let create_ten_ms = entry[13];
    let create_time = u16::from_le_bytes([entry[14], entry[15]]);
    let create_date = u16::from_le_bytes([entry[16], entry[17]]);
    let access_date = u16::from_le_bytes([entry[18], entry[19]]);
    let first_cluster_id = u32::from_le_bytes([entry[26], entry[27], entry[20], entry[21]]);
    let modify_time = u16::from_le_bytes([entry[22], entry[23]]);
    let modify_date = u16::from_le_bytes([entry[24], entry[25]]);
    let file_size = u32::from_le_bytes([entry[28], entry[29], entry[30], entry[31]]);
    Ok(DirEntry {
        short_name,
        long_name: CompactString::default(),
        attr,
        create_date,
        create_time,
        create_ten_ms,
        modify_date,
        modify_time,
        access_date,
        first_cluster_id,
        file_size,
    })
}

fn calc_checksum(short_name: &[u8; SNAME_MAX_LEN]) -> u8 {
    let mut checksum = 0u8;
    for &byte in short_name {
        checksum = (checksum >> 1) + ((checksum & 1) << 7);
        checksum = checksum.wrapping_add(byte);
    }
    checksum
}
