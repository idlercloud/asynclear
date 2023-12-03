OUTPUT_ARCH(riscv)
ENTRY(_start)
BASE_ADDRESS = 0xffffffff80200000;

SECTIONS
{
    . = BASE_ADDRESS;

    .text : {
        stext = .;
        __executable_start = .;
        *(.text.entry)
        *(.text .text.*)
        etext = .;
        __etext = .;
    }

    .rodata : ALIGN(4K) {
        srodata = .;
        *(.rodata .rodata.*)
        erodata = .;
    }

    .data : ALIGN(4K) {
        sdata = .;
        *(.data .data.*)
        edata = .;
    }

    .bss : {
	    *(.bss.stack)
        sbss = .;
        *(.sbss .bss .bss.*)
        ebss = .;
    }

    . = ALIGN(4K);
    ekernel = .;
    PROVIDE(end = .);

}