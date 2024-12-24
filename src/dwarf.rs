use gimli::{DW_AT_name, Dwarf};
use object::{self, Object, ObjectSection};
use std::error::Error;
use std::io::read_to_string;
use std::sync::Arc;
use libc::strcasecmp;
use crate::mmap;

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
pub type DwarfInfo = Dwarf<StaticEndianSlice>;

pub fn read_dwarf(filename: &str) -> Result<DwarfInfo, Box<dyn Error>> {
    let mut file = std::fs::File::open(filename)?;

    let mapping = unsafe { mmap::Mmap::map(&mut file) };
    let mmap_slice: &'static [u8] = mapping.leak();

    let elf = object::File::parse(mmap_slice)?;
    let endianness = Endianness::from(elf.endianness());

    let dwarf = gimli::Dwarf::load(|id| {
        match elf.section_by_name(id.name()) {
            Some(section) => Ok(gimli::EndianSlice::new(section.data()?, endianness)),
            // Some(section) => section.data(),
            None => Ok(gimli::EndianSlice::new(&EMPTY_ARR, endianness)),
        }
    });

    dwarf
}

fn identity<T>(t: T) -> T {
    t
}

pub fn process_dwarf<R>(dwarf: &mut DwarfInfo) -> Result<(), gimli::Error>
where
    R: gimli::Reader + Clone,
{
    println!("Processing DWARF sections:");

    let mut count = 0;

    let mut units = dwarf.units();

    while let Some(unit_header) = units.next().unwrap() {
        let unit = dwarf.unit(unit_header)?;
        let abbrev = Arc::clone(&unit.abbreviations);

        let mut entries = unit.entries();
        while let Some((_, dbg_entry)) = entries.next_dfs().unwrap() {
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

pub fn function_names_to_addresses(dwarf: &mut DwarfInfo, function: &str) -> Result<Vec<isize>, gimli::Error> {
    let mut units = dwarf.units();
    let mut addresses = Vec::new();

    while let Some(unit_header) = units.next()? {
        let unit = dwarf.unit(unit_header)?;
        // Traverse the DIEs
        let mut entries = unit.entries();
        while let Some((_, entry)) = entries.next_dfs()? {
            if entry.tag() != gimli::DW_TAG_subprogram && entry.tag() != gimli::DW_TAG_inlined_subroutine {
                continue;
            }
            // Check the name directly or via DW_AT_abstract_origin
            let Some(attr) = entry.attr_value(DW_AT_name)? else { continue; };
            let name: StaticEndianSlice = dwarf.attr_string(&unit, attr)?;

            // Regular function case
            let mut name_matches = name.to_string_lossy() == function;
            // Inlined case leads to abstract origin
            if !name_matches {
                if let Some(gimli::AttributeValue::UnitRef(abstract_origin))
                    = entry.attr_value(gimli::DW_AT_abstract_origin)?
                {
                    let origin_entry = unit.entry(abstract_origin)?;
                    if let Some(attr) = origin_entry.attr_value(DW_AT_name)? {
                        let name = dwarf.attr_string(&unit, attr)?;
                        name_matches = name.to_string_lossy() == function;
                    }
                }
            }
            if !name_matches { continue; }

            // If name matches, get the address
            if let Some(attr) = entry.attr_value(gimli::DW_AT_low_pc)? {
                if let gimli::AttributeValue::Addr(addr) = attr {
                    addresses.push(addr as isize);
                }
            }
        }
    }


    Ok(addresses)
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
