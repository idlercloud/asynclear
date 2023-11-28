    .section .text.entry
    .globl _start
_start:
    // a0 = hart id(0,1,2,...)
    // pc = 0x80200000（应该是由 qemu 决定的？）

    // 设置每个 hart 的 sp
    la sp, boot_stack_top
    slli t0, a0, 17 // t0 = hart_id * 4096 * 32（即左移 17 位）
                    // 每个 hart 栈为 4096 * 32 bytes
    sub sp, sp, t0  // sp = stack_top - hart_id * stack_size

    // 设置一个临时的 boot 用的页表。
    // 因为 linker 将内核链接在高地址，而 qemu 运行时实际上跑在低地址。
    // 如果指令中出现绝对地址就会有问题
    la   t0, boot_pt
    srli t0, t0, 12
    // 设置页表模式为 Sv39（即 satp 最高四位为 1000）
    li   t1, 0b1000 << 60
    or   t0, t0, t1
    csrw satp, t0
    sfence.vma

    la   t0, __hart_entry
    // 跳转到高地址（sp 也要设为高地址的）
    li   t1, 0xffffffff00000000
    add  t0, t0, t1
    add  sp, sp, t1
    jr   t0


    .section .bss.stack
    .align 12
boot_stack:
    .space 4096 * 32 * 8 // 暂时至多 8 harts
boot_stack_top:

    .section .data
    .align 12
boot_pt:
    // 虚拟地址                   物理地址
    // 0x0000_0000_8000_0000 -> 0x8000_0000 (1G, VRWXAD)
    // 0xffff_ffff_8000_0000 -> 0x8000_0000 (1G, VRWXAD)
                                    // PTE 项
    .zero 2 * 8                     // [0][1]
    .8byte (0x80000 << 10) | 0xcf   // [2]
    .zero 507 * 8                   // [3]~[509]
    .8byte (0x80000 << 10) | 0xcf   // [510]
    .zero 1 * 8                     // [511]
