.altmacro
.macro SAVE_REG prefix, n, m
    sd \prefix\n, \m*8(a0)
.endm
.macro LOAD_REG prefix, n, m
    ld \prefix\n, \m*8(a0)
.endm

.section .text.trap
.globl __trap_from_user
.globl __return_to_user
.globl __trap_from_kernel

.align 2
__trap_from_user:
    # 在这个情况下，TrapContext 的地址会被存放在 sscratch 中
    # 使 a0 指向 TrapContext，sscratch 暂存用户 a0 的值
    # 这里用 a0 只是为了和 __return_to_user 统一起来，用其他的也可以（记得改宏的定义）
    csrrw a0, sscratch, a0

    # 保存用户的通用寄存器，除去 a0(x10)，因为之后会保存
    # 保存 x1~x9
    .set n, 1
    .rept 9
        SAVE_REG x, %n, %(n-1)
        .set n, n+1
    .endr
    # 保存 x11~x31
    .set n, 11
    .rept 21
        SAVE_REG x, %n, %(n-1)
        .set n, n+1
    .endr

    # 现在可以自由使用 t0/t1/t2，因为它们已经保存在 TrapContext 中了
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 31*8(a0)
    sd t1, 32*8(a0)
    # 这时才保存用户的 a0（用户 a0 刚开始交换到 sscratch 里了）
    csrr t2, sscratch
    sd t2, 9*8(a0)

    # 恢复内核的上下文
    ld sp, 33*8(a0)
    ld ra, 34*8(a0)
    ld tp, 35*8(a0)
    .set n, 0
    .rept 12
        LOAD_REG s, %n, %(n+36)
        .set n, n+1
    .endr
    ret

__return_to_user:
    # a0: *mut TrapContext
    # 在 sscratch 中存放 TrapContext 的地址，下次陷入内核态可以使用
    csrw sscratch, a0

    # 保存内核上下文
    sd sp, 33*8(a0)
    sd ra, 34*8(a0)
    sd tp, 35*8(a0)
    .set n, 0
    .rept 12
        SAVE_REG s, %n, %(n+36)
        .set n, n+1
    .endr

    # 恢复 sstatus/sepc
    ld t0, 31*8(a0)
    ld t1, 32*8(a0)
    csrw sstatus, t0
    csrw sepc, t1
    # 恢复通用寄存器
    # 恢复 x1~x9
    .set n, 1
    .rept 9
        LOAD_REG x, %n, %(n-1)
        .set n, n+1
    .endr
    # 恢复 x11~x31
    .set n, 11
    .rept 21
        LOAD_REG x, %n, %(n-1)
        .set n, n+1
    .endr
    # 最后恢复 a0(x10) 寄存器
    ld a0, 9*8(a0)
    sret

.align 2
__trap_from_kernel:
    # 内核 trap 直接在内核栈上进行
    # 只需保存 caller-saved 的寄存器
    # 同时因为内核 trap 完成后必然回到原来的位置，且不会嵌套
    # sepc 和 sstatus 也不用保存
    addi sp, sp, -16*8
    sd  ra,  0*8(sp)
    sd  t0,  1*8(sp)
    sd  t1,  2*8(sp)
    sd  t2,  3*8(sp)
    sd  t3,  4*8(sp)
    sd  t4,  5*8(sp)
    sd  t5,  6*8(sp)
    sd  t6,  7*8(sp)
    sd  a0,  8*8(sp)
    sd  a1,  9*8(sp)
    sd  a2, 10*8(sp)
    sd  a3, 11*8(sp)
    sd  a4, 12*8(sp)
    sd  a5, 13*8(sp)
    sd  a6, 14*8(sp)
    sd  a7, 15*8(sp)
    call kernel_trap_handler
    ld  ra,  0*8(sp)
    ld  t0,  1*8(sp)
    ld  t1,  2*8(sp)
    ld  t2,  3*8(sp)
    ld  t3,  4*8(sp)
    ld  t4,  5*8(sp)
    ld  t5,  6*8(sp)
    ld  t6,  7*8(sp)
    ld  a0,  8*8(sp)
    ld  a1,  9*8(sp)
    ld  a2, 10*8(sp)
    ld  a3, 11*8(sp)
    ld  a4, 12*8(sp)
    ld  a5, 13*8(sp)
    ld  a6, 14*8(sp)
    ld  a7, 15*8(sp)
    addi sp, sp, 16*8
    sret

