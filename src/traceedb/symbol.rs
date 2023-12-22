use gimli::{self, Dwarf};

use object::{Object, ObjectSection};
use std::borrow;
use std::cell::Ref;
use std::error::Error;

// type TopLevelDwarfRef = Dwarf<EndianSlice<'dbg, RunTimeEndian>>;

pub fn load_dwarf_data(f_buf: &[u8]) -> Result<Dwarf<borrow::Cow<'_, [u8]>>, Box<dyn Error>> {
    let elf_obj = object::File::parse(&*f_buf)?;

    let section_loader = |section: gimli::SectionId| -> Result<borrow::Cow<[u8]>, gimli::Error> {
        match elf_obj.section_by_name(section.name()) {
            Some(ref section) => Ok(section
                .uncompressed_data()
                .unwrap_or(borrow::Cow::Borrowed(&[][..]))),
            None => Ok(borrow::Cow::Borrowed(&[][..])),
        }
    };

    let dwarf_cow = gimli::Dwarf::load(&section_loader)?;

    Ok(dwarf_cow)
}

pub fn src_line_to_addr(
    dwarf_cow: Ref<'_, Dwarf<borrow::Cow<'_, [u8]>>>,
    filename: &str,
    line_num: u64,
) -> Result<u64, Box<dyn Error>> {
    let dwarf = dwarf_cow
        .borrow(|section| gimli::EndianSlice::new(&*section, gimli::RunTimeEndian::Little));

    let mut iter = dwarf.units();
    while let Some(header) = iter.next()? {
        let unit = dwarf.unit(header)?;

        // Iterate over the Debugging Information Entries (DIEs) in the unit.
        let mut entries = unit.entries();

        if let Some((_depth, top_die)) = entries.next_dfs()? {
            if let (Ok(Some(_comp_dir_atval)), Ok(Some(name_atval))) = (
                top_die.attr_value(gimli::DwAt(0x1b)),
                top_die.attr_value(gimli::DwAt(0x03)),
            ) {
                let cu_name = dwarf.attr_string(&unit, name_atval)?.to_string()?;

                if cu_name == filename {
                    let line_prog = unit.line_program.unwrap();
                    let mut rows = line_prog.rows();

                    while let Ok(Some((_header, row))) = rows.next_row() {
                        if let Some(l) = row.line().map(u64::from) {
                            if l == line_num {
                                return Ok(row.address());
                            }
                        }
                    }
                }
            }
        }
    }

    Err(Box::new(gimli::Error::InvalidAddressRange))
}
