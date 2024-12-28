use std::borrow::Cow;
use crate::mmap;
use gimli;
use object::{self, Object, ObjectSection};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;

const EMPTY_ARR: [u8; 0] = [];

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Endianness {
    Little,
    Big,
}

impl From<object::Endianness> for Endianness {
    fn from(endianness: object::Endianness) -> Self {
        match endianness {
            object::Endianness::Little => Self::Little,
            object::Endianness::Big => Self::Big,
        }
    }
}

impl gimli::Endianity for Endianness {
    fn is_big_endian(self) -> bool {
        matches!(self, Endianness::Big)
    }
}

pub type StaticEndianSlice = gimli::EndianSlice<'static, Endianness>;
pub type Dwarf = gimli::Dwarf<StaticEndianSlice>;

pub struct DwarfInfo {
    pub dwarf: Dwarf,
    function_cache: HashMap<String, Vec<u64>>,
}

impl DwarfInfo {
    pub fn new(dwarf: Dwarf) -> Self {
        Self {
            dwarf,
            function_cache: HashMap::new(),
        }
    }

    pub fn function_addresses(&mut self, function: &str) -> Result<Vec<u64>, gimli::Error> {
        if let Some(addresses) = self.function_cache.get(function).cloned() {
            return Ok(addresses);
        }
        let addresses = function_names_to_addresses(&mut self.dwarf, function)?;
        dbg!(&addresses);

        self.function_cache.insert(function.to_string(), addresses);
        Ok(self
            .function_cache
            .get(function)
            .expect("Just inserted")
            .clone())
    }
}

pub fn read_dwarf(filename: &str) -> Result<Dwarf, Box<dyn Error>> {
    let mut file = std::fs::File::open(filename)?;

    let mapping = unsafe { mmap::Mmap::map(&mut file) };
    let mmap_slice: &'static [u8] = mapping.leak();

    let elf = object::File::parse(mmap_slice)?;
    let endianness = Endianness::from(elf.endianness());

    let dwarf = gimli::Dwarf::load(|id| match elf.section_by_name(id.name()) {
        Some(section) => Ok(gimli::EndianSlice::new(section.data()?, endianness)),
        None => Ok(gimli::EndianSlice::new(&EMPTY_ARR, endianness)),
    });

    dwarf
}

pub fn process_dwarf_test<R>(dwarf: &mut Dwarf) -> Result<(), gimli::Error>
where
    R: gimli::Reader + Clone,
{
    println!("Processing DWARF sections:");

    let mut count = 0;

    let mut units = dwarf.units();

    while let Some(unit_header) = units.next().unwrap() {
        let unit = dwarf.unit(unit_header)?;

        let mut entries = unit.entries();
        while let Some((_, dbg_entry)) = entries.next_dfs()? {
            if let Some(attr) = dbg_entry.attr_value(gimli::DW_AT_name)? {
                match dwarf.attr_string(&unit, attr) {
                    Ok(name) => {
                        println!("Name: {:?}, {}", name.to_string_lossy(), dbg_entry.tag());
                    }
                    Err(e) => {
                        println!("Error: {:?}", e);
                    }
                }
            }
        }
        count += 1;
    }

    println!("Done processing DWARF sections. Total: {}", count);

    Ok(())
}

pub fn function_names_to_addresses(
    dwarf: &mut Dwarf,
    function: &str,
) -> Result<Vec<u64>, gimli::Error> {
    let mut units = dwarf.units();
    let mut addresses = Vec::new();

    let mut add_entry_address =
        |entry: &gimli::DebuggingInformationEntry<StaticEndianSlice>| -> Result<(), gimli::Error> {
            if let Some(gimli::AttributeValue::Addr(addr)) =
                entry.attr_value(gimli::DW_AT_low_pc)?
            {
                addresses.push(addr);
            }
            Ok(())
        };

    while let Some(unit_header) = units.next()? {
        let unit = dwarf.unit(unit_header)?;
        // Traverse the DIEs
        let mut entries = unit.entries();
        while let Some((_, entry)) = entries.next_dfs()? {
            match entry.tag() {
                gimli::DW_TAG_subprogram => {
                    // A fully fledged function *should* be named
                    let Some(attr) = entry.attr_value(gimli::DW_AT_name)? else {
                        continue;
                    };
                    let name: StaticEndianSlice = dwarf.attr_string(&unit, attr)?;

                    if name.to_string_lossy() == function {
                        add_entry_address(entry)?;
                    }
                }
                gimli::DW_TAG_inlined_subroutine => {
                    // Inlined functions will have abstract origin we need to look up to find name
                    if let Some(gimli::AttributeValue::UnitRef(abstract_origin)) =
                        entry.attr_value(gimli::DW_AT_abstract_origin)?
                    {
                        let origin_entry = unit.entry(abstract_origin)?;
                        if let Some(attr) = origin_entry.attr_value(gimli::DW_AT_name)? {
                            if dwarf.attr_string(&unit, attr)?.to_string_lossy() != function {
                                continue;
                            }
                            add_entry_address(entry)?;
                        }
                    }
                }
                _ => (),
            }
        }
    }

    Ok(addresses)
}


pub fn find_function_at_pc(
    dwarf: &Dwarf,
    pc: u64,
    base: u64,
) -> Result<Option<CodePoint>, gimli::Error> {
    let mut units = dwarf.units();

    while let Some(header) = units.next()? {
        let unit: gimli::Unit<StaticEndianSlice> = dwarf.unit(header)?;
        let mut entries = unit.entries();

        let mut best_func: Option<StaticEndianSlice> = None;
        let mut best_range: Option<(u64, u64)> = None;
        while let Some((_, entry)) = entries.next_dfs()? {
            if ![gimli::DW_TAG_subprogram, gimli::DW_TAG_inlined_subroutine].contains(&entry.tag())
            {
                continue;
            }
            let Some(function_name) = function_name_from_entry(dwarf, &unit, entry)? else {
                continue;
            };
            let Some((lo_pc_offset, hi_pc_offset)) = function_lo_hi_pc(entry)? else {
                continue;
            };

            let (lo_pc, hi_pc) = (lo_pc_offset + base, hi_pc_offset + base);
            if !(lo_pc..hi_pc).contains(&pc) || best_range.is_some_and(|(lo, _)| lo > lo_pc) {
                continue;
            }
            best_range = Some((lo_pc, hi_pc));
            best_func = Some(function_name);
        }
        if best_range.is_none() {
            // No codepoint found
            continue;
        }

        let Some((header, row)) = find_row_at_pc(&unit, pc, base)? else {
            continue;
        };

        return Ok(Some(CodePoint {
            row,
            real_addr: pc,
            file: header
                .file(row.file_index())
                .and_then(|file_entry| dwarf.attr_string(&unit, file_entry.path_name()).ok()),
            function: best_func
        }));
    }

    Ok(None)
}

fn find_row_at_pc(
    unit: &gimli::Unit<StaticEndianSlice>,
    pc: u64,
    base: u64,
) -> Result<Option<(gimli::LineProgramHeader<StaticEndianSlice>, gimli::LineRow)>, gimli::Error> {
    let Some(program) = unit.line_program.as_ref() else {
        return Ok(None);
    };
    let mut rows = program.clone().rows();
    let mut best_row: Option<(_, gimli::LineRow)> = None;

    while let Some((header, row)) = rows.next_row()? {
        if row.address() + base > pc
            || row.end_sequence()
            || best_row
                .as_ref()
                .is_some_and(|(_, best_row)| best_row.address() > row.address())
        {
            continue;
        }
        best_row = Some((header.clone(), *row));
    }

    Ok(best_row)
}

impl Default for Endianness {
    fn default() -> Self {
        Self::native()
    }
}

impl Endianness {
    const fn native_is_little_endian() -> bool {
        u32::from_ne_bytes([1, 0, 0, 0]) == 1
    }

    const fn native() -> Self {
        match Self::native_is_little_endian() {
            true => Endianness::Little,
            false => Endianness::Big,
        }
    }
}

fn function_name_from_entry(
    dwarf: &Dwarf,
    unit: &gimli::Unit<StaticEndianSlice>,
    entry: &gimli::DebuggingInformationEntry<StaticEndianSlice>,
) -> Result<Option<StaticEndianSlice>, gimli::Error> {
    if let Some(attr) = entry.attr_value(gimli::DW_AT_name)? {
        let name = dwarf.attr_string(unit, attr)?;
        return Ok(Some(name));
    }

    // Check if it's an inlined function by looking at abstract origin
    if entry.tag() == gimli::DW_TAG_inlined_subroutine {
        if let Some(gimli::AttributeValue::UnitRef(abstract_origin)) =
            entry.attr_value(gimli::DW_AT_abstract_origin)?
        {
            let origin_entry = unit.entry(abstract_origin)?;
            if let Some(attr) = origin_entry.attr_value(gimli::DW_AT_name)? {
                let name = dwarf.attr_string(unit, attr)?;
                return Ok(Some(name));
            }
        }
    }
    Ok(None)
}

fn function_lo_hi_pc(
    entry: &gimli::DebuggingInformationEntry<StaticEndianSlice>,
) -> gimli::Result<Option<(u64, u64)>> {
    let lo = match entry.attr_value(gimli::DW_AT_low_pc)? {
        Some(gimli::AttributeValue::Addr(lo)) => lo,
        _ => return Ok(None),
    };

    let hi = match entry.attr_value(gimli::DW_AT_high_pc)? {
        Some(gimli::AttributeValue::Addr(hi)) => hi,
        Some(gimli::AttributeValue::Udata(hi)) => lo + hi,
        Some(gimli::AttributeValue::Data1(hi)) => lo + hi as u64,
        Some(gimli::AttributeValue::Data2(hi)) => lo + hi as u64,
        Some(gimli::AttributeValue::Data4(hi)) => lo + hi as u64,
        Some(gimli::AttributeValue::Data8(hi)) => lo + hi,
        Some(gimli::AttributeValue::Sdata(hi)) if hi < 0 => {
            return Err(gimli::Error::InvalidAddressRange)
        }
        Some(gimli::AttributeValue::Sdata(hi)) => lo + hi as u64,
        _ => return Ok(None),
    };

    Ok(Some((lo, hi)))
}

#[derive(Debug, Clone)]
pub struct CodePoint {
    pub row: gimli::LineRow,
    pub real_addr: u64,
    pub file: Option<StaticEndianSlice>,
    pub function: Option<StaticEndianSlice>,
}

impl Display for CodePoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let filename = static_endian_slice_to_string_lossy_or(self.file.as_ref(), "<unknown_file>");
        let func = static_endian_slice_to_string_lossy_or(self.function.as_ref(), "<unknown_function>");
        let x = write!(
            f,
            "CodePoint({}:{}@{})",
            filename,
            self.row.line().map(u64::from).unwrap_or(0),
            func
        );
        x
    }
}

fn static_endian_slice_to_string_lossy_or(ses: Option<&StaticEndianSlice>, default: &'static str) -> Cow<'static, str> {
    ses.map_or_else(
        || Cow::Borrowed(default),
        |ses| ses.to_string_lossy()
    )
}
