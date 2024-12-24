use std::collections::HashMap;
use gimli;
use object::{self, Object, ObjectSection};
use std::error::Error;
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
pub type Dwarf = gimli::Dwarf<StaticEndianSlice>;

pub struct DwarfInfo {
    dwarf: Dwarf,
    function_cache: HashMap<String, Vec<isize>>,
}

impl DwarfInfo {
    pub fn new(dwarf: Dwarf) -> Self {
        Self {
            dwarf,
            function_cache: HashMap::new(),
        }
    }

    pub fn function_addresses(&mut self, function: &str) -> Result<Vec<isize>, gimli::Error> {
        if let Some(addresses) = self.function_cache.get(function).cloned() {
            return Ok(addresses);
        }
        let addresses: Vec<isize> = function_names_to_addresses(&mut self.dwarf, function)?;

        self.function_cache.insert(function.to_string(), addresses);
        Ok(self.function_cache.get(function).expect("Just inserted").clone())
    }
}

pub fn read_dwarf(filename: &str) -> Result<Dwarf, Box<dyn Error>> {
    let mut file = std::fs::File::open(filename)?;

    let mapping = unsafe { mmap::Mmap::map(&mut file) };
    let mmap_slice: &'static [u8] = mapping.leak();

    let elf = object::File::parse(mmap_slice)?;
    let endianness = Endianness::from(elf.endianness());

    let dwarf = gimli::Dwarf::load(|id| {
        match elf.section_by_name(id.name()) {
            Some(section) => Ok(gimli::EndianSlice::new(section.data()?, endianness)),
            None => Ok(gimli::EndianSlice::new(&EMPTY_ARR, endianness)),
        }
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

pub fn function_names_to_addresses(dwarf: &mut Dwarf, function: &str) -> Result<Vec<isize>, gimli::Error> {
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
            let Some(attr) = entry.attr_value(gimli::DW_AT_name)? else { continue; };
            let name: StaticEndianSlice = dwarf.attr_string(&unit, attr)?;

            // Regular function case
            let mut name_matches = name.to_string_lossy() == function;
            // Inlined case leads to abstract origin
            if !name_matches {
                if let Some(gimli::AttributeValue::UnitRef(abstract_origin))
                    = entry.attr_value(gimli::DW_AT_abstract_origin)?
                {
                    let origin_entry = unit.entry(abstract_origin)?;
                    if let Some(attr) = origin_entry.attr_value(gimli::DW_AT_name)? {
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
