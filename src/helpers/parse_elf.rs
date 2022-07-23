//! Copyright (c) ChefKiss Inc 2021-2022.
//! This project is licensed by the Creative Commons Attribution-NoCommercial-NoDerivatives license.

use log::debug;

pub fn parse_elf(
    mem_mgr: &mut super::mem::MemoryManager,
    buffer: &[u8],
) -> sulfur_dioxide::EntryPoint {
    let elf = goblin::elf::Elf::parse(buffer).expect("Failed to parse kernel elf");

    debug!("{:X?}", elf.header);
    assert!(elf.is_64, "Only ELF64");
    assert_eq!(elf.header.e_machine, goblin::elf::header::EM_X86_64);
    assert!(elf.little_endian, "Only little-endian ELFs");
    assert!(
        elf.entry as usize >= amd64::paging::KERNEL_VIRT_OFFSET,
        "Only higher-half kernels"
    );

    debug!("Parsing program headers: ");
    for phdr in elf
        .program_headers
        .iter()
        .filter(|phdr| phdr.p_type == goblin::elf::program_header::PT_LOAD)
    {
        assert!(
            phdr.p_vaddr as usize >= amd64::paging::KERNEL_VIRT_OFFSET,
            "Only higher-half kernels."
        );

        let offset = phdr.p_offset as usize;
        let memsz = phdr.p_memsz as usize;
        let file_size = phdr.p_filesz as usize;
        let src = &buffer[offset..(offset + file_size)];
        let dest = unsafe {
            core::slice::from_raw_parts_mut(
                (phdr.p_vaddr as usize - amd64::paging::KERNEL_VIRT_OFFSET) as *mut u8,
                memsz,
            )
        };
        let npages = (memsz + 0xFFF) / 0x1000;
        debug!(
            "vaddr: {:#X}, paddr: {:#X}, npages: {:#X}",
            phdr.p_vaddr,
            phdr.p_vaddr as usize - amd64::paging::KERNEL_VIRT_OFFSET,
            npages
        );
        assert_eq!(
            unsafe { uefi_services::system_table().as_mut() }
                .boot_services()
                .allocate_pages(
                    uefi::table::boot::AllocateType::Address(
                        phdr.p_vaddr as usize - amd64::paging::KERNEL_VIRT_OFFSET,
                    ),
                    uefi::table::boot::MemoryType::LOADER_DATA,
                    npages,
                )
                .expect("Failed to load section above. Sections might be misaligned.")
                as usize,
            phdr.p_vaddr as usize - amd64::paging::KERNEL_VIRT_OFFSET
        );

        mem_mgr.allocate((
            phdr.p_vaddr as usize - amd64::paging::KERNEL_VIRT_OFFSET,
            npages,
        ));

        for (a, b) in dest
            .iter_mut()
            .zip(src.iter().chain(core::iter::repeat(&0)))
        {
            *a = *b
        }
    }

    unsafe { core::mem::transmute(elf.entry as *const ()) }
}
