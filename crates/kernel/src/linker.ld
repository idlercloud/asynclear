OUTPUT_ARCH(riscv)
ENTRY(_start)
BASE_ADDRESS = 0xffffffff80200000;

SECTIONS
{
    . = BASE_ADDRESS;

    .text : {
        stext = .;
        *(.text.entry)
        *(.text .text.*)
    }

    .rodata : ALIGN(4K) {
        etext = .;
        srodata = .;
        *(.rodata .rodata.*)
    }

    .data : ALIGN(4K) {
        erodata = .;
        sdata = .;
        *(.data .data.*)
    }

    .stack : ALIGN(4K) {
        edata = .;
        sstack = .;
        *(.bss.stack)
    }

    .bss : ALIGN(4K) {
        estack = .;
        sbss = .;
        *(.sbss .bss .bss.*)
    }

    . = ALIGN(4K);
    ebss = .;
    ekernel = .;
    PROVIDE(end = .);

}