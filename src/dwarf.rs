use crate::mmap;
use gimli;
use object::{self, Object, ObjectSection};
use std::collections::HashMap;
use std::error::Error;

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

    let mut add_entry_address = |entry: &gimli::DebuggingInformationEntry<StaticEndianSlice>|
     -> Result<(), gimli::Error> {
        if let Some(gimli::AttributeValue::Addr(addr)) = entry.attr_value(gimli::DW_AT_low_pc)? {
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
                        println!("noname function");
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

// pub fn address_to_function(dwarf: &mut Dwarf, pc: isize) -> Result<(), gimli::Error> {
//     let mut units = dwarf.units();
//
//     while let Some(unit_header) = units.next()? {
//         let unit = dwarf.unit(unit_header)?;
//         // Traverse the DIEs
//         let mut entries = unit.entries();
//         while let Some((_, entry)) = entries.next_dfs()? {
//             // Note that this only deals with top-level functions
//             // This won't do anything for inlined entries
//             if entry.tag() != gimli::DW_TAG_subprogram {
//                 continue;
//             }
//         let Some(attr_lo) = entry.attr_value(gimli::DW_AT_low_pc)? else { continue; };
//             let gimli::AttributeValue::Addr(addr) = attr_lo else { continue; };
//             if addr as isize == pc {
//                 println!("Found function at address: {:?}", pc);
//                 return Ok(());
//             }
//         }
//     }
//
//     todo!()
// }

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
